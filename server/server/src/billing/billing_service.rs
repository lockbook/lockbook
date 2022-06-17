use crate::account_service::GetUsageHelperError;
use crate::billing::billing_model::{BillingPlatform, GooglePlayUserInfo, SubscriptionProfile};
use crate::billing::google_play_model::NotificationType;
use crate::billing::{google_play_client, google_play_service, stripe_client, stripe_service};
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, keys, RequestContext, ServerError, ServerState, FREE_TIER_USAGE_SIZE,
    PREMIUM_TIER_USAGE_SIZE,
};
use base64::DecodeError;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, CancelSubscriptionResponse,
    GetSubscriptionInfoError, GetSubscriptionInfoRequest, GetSubscriptionInfoResponse,
    GooglePlayAccountState, PaymentPlatform, SubscriptionInfo, UpgradeAccountGooglePlayError,
    UpgradeAccountGooglePlayRequest, UpgradeAccountGooglePlayResponse, UpgradeAccountStripeError,
    UpgradeAccountStripeRequest, UpgradeAccountStripeResponse,
};
use log::{info, warn};
use redis_utils::converters::{JsonGet, JsonSet, PipelineJsonSet};
use redis_utils::tx;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

#[derive(Debug)]
pub enum LockBillingWorkflowError {
    ExistingRequestPending,
}

async fn lock_subscription_profile(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection, millis_between_payment_flows: u64,
) -> Result<SubscriptionProfile, ServerError<LockBillingWorkflowError>> {
    let mut sub_profile = SubscriptionProfile::default();

    let tx_result = tx!(con, pipe, &[keys::subscription_profile(public_key)], {
        sub_profile = con
            .maybe_json_get(keys::subscription_profile(public_key))
            .await?
            .unwrap_or_default();

        let current_time = get_time().0 as u64;

        if current_time - sub_profile.last_in_payment_flow < millis_between_payment_flows {
            warn!(
                "User/Webhook is already in payment flow, or not enough time that has elapsed since a failed attempt. public_key: {}",
                keys::stringify_public_key(public_key)
            );

            return Err(Abort(ClientError(LockBillingWorkflowError::ExistingRequestPending)));
        }

        sub_profile.last_in_payment_flow = current_time;

        pipe.json_set(keys::subscription_profile(public_key), &sub_profile)
    });
    return_if_error!(tx_result);

    info!(
        "User successfully entered payment flow. public_key: {}",
        keys::stringify_public_key(public_key)
    );

    Ok(sub_profile)
}

async fn release_subscription_profile<E: Debug>(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection,
    sub_profile: &mut SubscriptionProfile,
) -> Result<(), ServerError<E>> {
    sub_profile.last_in_payment_flow = 0;
    Ok(con
        .json_set(keys::subscription_profile(public_key), sub_profile)
        .await?)
}

pub async fn upgrade_account_google_play(
    context: RequestContext<'_, UpgradeAccountGooglePlayRequest>,
) -> Result<UpgradeAccountGooglePlayResponse, ServerError<UpgradeAccountGooglePlayError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_profile = lock_subscription_profile(
        &context.public_key,
        &mut con,
        server_state
            .config
            .billing
            .millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(UpgradeAccountGooglePlayError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_profile.data_cap() == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountGooglePlayError::AlreadyPremium));
    }

    info!(
        "Upgrading the account of a user through google play billing. public_key: {:?}.",
        context.public_key
    );

    google_play_client::acknowledge_subscription(
        &server_state.google_play_client,
        &server_state
            .config
            .billing
            .gp_premium_subscription_product_id,
        &request.purchase_token,
    )
    .await?;

    info!("Acknowledged a user's google play subscription. public_key {:?}", context.public_key);

    let subscription = google_play_client::get_subscription(
        &server_state.google_play_client,
        &server_state
            .config
            .billing
            .gp_premium_subscription_product_id,
        &request.purchase_token,
    )
    .await?;

    subscription.sub_profile.billing_platform =
        Some(BillingPlatform::GooglePlay(GooglePlayUserInfo {
            purchase_token: request.purchase_token.clone(),
            subscription_product_id: server_state
                .config
                .billing
                .gp_premium_subscription_product_id
                .clone(),
            subscription_offer_id: server_state
                .config
                .billing
                .gp_premium_subscription_offer_id
                .clone(),
            expiration_time: subscription
                .expiry_time_millis
                .ok_or_else(|| {
                    internal!(
                        "Cannot get expiration time of a recovered subscription. public_key {:?}",
                        &context.public_key
                    )
                })?
                .parse()
                .map_err(|e| internal!("Cannot parse millis into int: {:?}", e))?,
            account_state: GooglePlayAccountState::Ok,
        }));

    con.json_set(keys::public_key_from_gp_account_id(&request.account_id), context.public_key)
        .await?;
    release_subscription_profile(&context.public_key, &mut con, &mut sub_profile).await?;

    info!(
        "Successfully upgraded a user through a google play subscription. public_key: {:?}",
        context.public_key
    );

    Ok(UpgradeAccountGooglePlayResponse {})
}

pub async fn upgrade_account_stripe(
    context: RequestContext<'_, UpgradeAccountStripeRequest>,
) -> Result<UpgradeAccountStripeResponse, ServerError<UpgradeAccountStripeError>> {
    let (request, server_state) = (&context.request, context.server_state);

    info!("Attempting to upgrade the account tier of {:?} to premium", context.public_key);

    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_profile = lock_subscription_profile(
        &context.public_key,
        &mut con,
        server_state
            .config
            .billing
            .millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(UpgradeAccountStripeError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_profile.data_cap() == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountStripeError::AlreadyPremium));
    }

    let maybe_user_info = sub_profile.billing_platform.and_then(|info| match info {
        BillingPlatform::Stripe(stripe_info) => Some(stripe_info),
        _ => None,
    });

    let user_info = stripe_service::create_subscription(
        server_state,
        &mut con,
        &context.public_key,
        &request.account_tier,
        maybe_user_info,
    )
    .await?;

    sub_profile.billing_platform = Some(BillingPlatform::Stripe(user_info));
    release_subscription_profile(&context.public_key, &mut con, &mut sub_profile).await?;

    info!(
        "Successfully upgraded the account tier of {:?} from free to premium.",
        context.public_key
    );

    Ok(UpgradeAccountStripeResponse {})
}

pub async fn get_subscription_info(
    context: RequestContext<'_, GetSubscriptionInfoRequest>,
) -> Result<GetSubscriptionInfoResponse, ServerError<GetSubscriptionInfoError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let sub_profile: SubscriptionProfile = match con
        .maybe_json_get(keys::subscription_profile(&context.public_key))
        .await?
    {
        Some(sub_profile) => sub_profile,
        None => return Ok(GetSubscriptionInfoResponse { subscription_info: None }),
    };

    let subscription_info = sub_profile.billing_platform.map(|info| match info {
        BillingPlatform::Stripe(info) => SubscriptionInfo {
            payment_platform: PaymentPlatform::Stripe { card_last_4_digits: info.last_4.clone() },
            period_end: info.expiration_time,
        },
        BillingPlatform::GooglePlay(info) => SubscriptionInfo {
            payment_platform: PaymentPlatform::GooglePlay {
                account_state: info.account_state.clone(),
            },
            period_end: info.expiration_time,
        },
    });

    Ok(GetSubscriptionInfoResponse { subscription_info })
}

pub async fn cancel_subscription(
    context: RequestContext<'_, CancelSubscriptionRequest>,
) -> Result<CancelSubscriptionResponse, ServerError<CancelSubscriptionError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_profile = lock_subscription_profile(
        &context.public_key,
        &mut con,
        server_state
            .config
            .billing
            .millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(CancelSubscriptionError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_profile.data_cap() == FREE_TIER_USAGE_SIZE {
        return Err(ClientError(CancelSubscriptionError::NotPremium));
    }

    let usage: u64 = account_service::get_usage_helper(&mut con, &context.public_key)
        .await
        .map_err(|e| match e {
            GetUsageHelperError::UserNotFound => ClientError(CancelSubscriptionError::NotPremium),
            GetUsageHelperError::Internal(e) => ServerError::from(e),
        })?
        .iter()
        .map(|a| a.size_bytes)
        .sum();

    if usage > FREE_TIER_USAGE_SIZE {
        info!(
            "Cannot downgrade user to free since they are over the data cap. public_key: {:?}",
            context.public_key
        );
        return Err(ClientError(CancelSubscriptionError::UsageIsOverFreeTierDataCap));
    }

    match sub_profile.billing_platform {
        None => return Err(internal!("A user somehow has premium tier usage, but no billing information on redis. public_key: {:?}", context.public_key)),
        Some(BillingPlatform::GooglePlay(ref mut info)) => {
            info!("Canceling google play subscription of user. public_key: {:?}.", context.public_key);

            if let GooglePlayAccountState::Canceled = &info.account_state {
                return Err(ClientError(CancelSubscriptionError::NotPremium))
            }

            google_play_client::cancel_subscription(
                &server_state.google_play_client,
                &info.subscription_product_id,
                &info.purchase_token,
            ).await?;

            info.account_state = GooglePlayAccountState::Canceled;
            info!("Successfully canceled google play subscription of user. public_key: {:?}.", context.public_key);
        }
        Some(BillingPlatform::Stripe(ref info)) => {
            info!("Canceling stripe subscription of user. public_key: {:?}.", context.public_key);

            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &info.subscription_id.parse()?,
            )
                .await
                .map_err(|err| internal!("{:?}", err))?;

            sub_profile.billing_platform = None;

            info!("Successfully canceled stripe subscription. public_key: {:?}", context.public_key);
        }
    }

    release_subscription_profile(&context.public_key, &mut con, &mut sub_profile).await?;

    Ok(CancelSubscriptionResponse {})
}

async fn save_subscription_profile<
    T: Debug,
    F: Fn(&mut SubscriptionProfile) -> Result<(), ServerError<T>>,
>(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection,
    millis_between_payment_flows: u64, millis_between_lock_attempts: Duration,
    update_subscription_profile: F,
) -> Result<(), ServerError<T>> {
    loop {
        match lock_subscription_profile(public_key, con, millis_between_payment_flows).await {
            Ok(ref mut sub_profile) => {
                update_subscription_profile(sub_profile)?;
                release_subscription_profile(public_key, con, sub_profile).await?;
                break;
            }
            Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => {
                tokio::time::sleep(millis_between_lock_attempts).await;
                continue;
            }
            Err(err) => return Err(internal!("Cannot get billing lock in webhooks: {:#?}", err)),
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum StripeWebhookError {
    VerificationError(String),
    InvalidHeader(String),
    InvalidBody(String),
    ParseError(String),
}

pub async fn stripe_webhooks(
    server_state: &Arc<ServerState>, request_body: Bytes, stripe_sig: HeaderValue,
) -> Result<(), ServerError<StripeWebhookError>> {
    let event =
        stripe_service::verify_request_and_get_event(server_state, &request_body, stripe_sig)?;

    info!("Verified stripe request. event: {:?}.", event.event_type);

    let mut con = server_state.index_db_pool.get().await?;

    match (&event.event_type, &event.data.object) {
        (stripe::EventType::InvoicePaymentFailed, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = stripe_service::get_public_key(&mut con, invoice).await?;

                info!(
                    "User's tier is being reduced due to failed renewal payment in stripe. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );

                save_subscription_profile(
                    &public_key,
                    &mut con,
                    server_state
                        .config
                        .billing
                        .millis_between_user_payment_flows,
                    server_state.config.billing.time_between_lock_attempts,
                    |sub_profile| {
                        sub_profile.billing_platform = None;
                        Ok(())
                    },
                )
                .await?;
            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = stripe_service::get_public_key(&mut con, invoice).await?;

                info!(
                    "User's subscription period_end is being changed after successful renewal. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );

                let subscription_period_end = match &invoice.subscription {
                    Some(stripe::Expandable::Object(subscription)) => {
                        subscription.current_period_end
                    }
                    _ => {
                        return Err(internal!(
                            "The subscription should be expanded in this invoice: {:?}",
                            invoice
                        ));
                    }
                };

                save_subscription_profile(
                    &public_key,
                    &mut con,
                    server_state
                        .config
                        .billing
                        .millis_between_user_payment_flows,
                    server_state.config.billing.time_between_lock_attempts,
                    |sub_profile| {
                        if let Some(BillingPlatform::Stripe(ref mut info)) =
                            sub_profile.billing_platform
                        {
                            info.expiration_time = subscription_period_end as u64;
                        }
                        Ok(())
                    },
                )
                .await?;
            }
        }
        (_, _) => {
            return Err(internal!("Unexpected stripe event: {:?}", event.event_type));
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum GooglePlayWebhookError {
    InvalidToken,
    CannotRetrieveData,
    CannotDecodePubSubData(DecodeError),
    CannotRetrieveUserInfo,
    CannotRetrievePublicKey,
    CannotParseTime,
}

pub async fn google_play_notification_webhooks(
    server_state: &Arc<ServerState>, request_body: Bytes, query_parameters: HashMap<String, String>,
) -> Result<(), ServerError<GooglePlayWebhookError>> {
    let notification = google_play_service::verify_request_and_get_notification(
        server_state,
        request_body,
        query_parameters,
    )
    .await?;

    let mut con = server_state.index_db_pool.get().await?;

    if let Some(sub_notif) = notification.subscription_notification {
        info!("Notification is for a subscription: {:?}", sub_notif);

        let subscription = google_play_client::get_subscription(
            &server_state.google_play_client,
            &sub_notif.subscription_id,
            &sub_notif.purchase_token,
        )
        .await
        .map_err(|e| internal!("{:#?}", e))?;

        let notification_type = sub_notif.notification_type();
        if let NotificationType::SubscriptionPurchased = notification_type {
            return Ok(());
        }

        let public_key = google_play_service::get_public_key(
            &mut con,
            &sub_notif,
            &subscription,
            &notification_type,
        )
        .await?;

        info!("Updating google play user's subscription profile to match new subscription state. public_key: {:?}, notification_type: {:?}", public_key, notification_type);

        save_subscription_profile(
            &public_key,
            &mut con,
            server_state
                .config
                .billing
                .millis_between_user_payment_flows,
            server_state.config.billing.time_between_lock_attempts,
            |sub_profile| {
                if let Some(BillingPlatform::GooglePlay(ref mut info)) = sub_profile.billing_platform {
                    match notification_type {
                        NotificationType::SubscriptionRecovered
                        | NotificationType::SubscriptionRestarted
                        | NotificationType::SubscriptionRenewed => {
                            info.account_state = GooglePlayAccountState::Ok;
                            info.expiration_time =
                                google_play_service::get_subscription_period_end(
                                    &subscription,
                                    &notification_type,
                                    public_key,
                                )?;
                        }
                        NotificationType::SubscriptionInGracePeriod => {
                            info.account_state = GooglePlayAccountState::GracePeriod;
                        }
                        NotificationType::SubscriptionOnHold => {
                            info.account_state = GooglePlayAccountState::OnHold;
                            info.expiration_time =
                                google_play_service::get_subscription_period_end(
                                    &subscription,
                                    &notification_type,
                                    public_key,
                                )?;
                        }
                        NotificationType::SubscriptionExpired
                        | NotificationType::SubscriptionRevoked => {
                            if info.purchase_token == sub_notif.purchase_token {
                                sub_profile.billing_platform = None
                            } else {
                                info!("Expired or revoked subscription was tied to an old purchase_token. old purchase_token: {:?}, new purchase_token: {:?}", sub_notif.purchase_token, info.purchase_token);
                            }
                        }
                        NotificationType::SubscriptionCanceled => {
                            info.account_state = GooglePlayAccountState::Canceled;
                            info!(
                                "Reason of cancellation: {:?}, public_key: {:?}",
                                subscription.cancel_survey_result,
                                public_key
                            );
                        }
                        NotificationType::SubscriptionPriceChangeConfirmed
                        | NotificationType::SubscriptionDeferred
                        | NotificationType::SubscriptionPaused
                        | NotificationType::SubscriptionPausedScheduleChanged
                        | NotificationType::SubscriptionPurchased => {
                            return Err(internal!(
                                "Unexpected subscription notification: {:?}, public_key: {:?}",
                                notification_type,
                                public_key
                            ))
                        }
                        NotificationType::Unknown => {
                            return Err(internal!(
                                "Unknown subscription change. public_key: {:?}",
                                public_key
                            ))
                        }
                    }

                    Ok(())
                } else {
                    Err(internal!(
                        "Cannot get any billing info for user. public_key: {:?}",
                        public_key
                    ))
                }
            }
        ).await?;
    }

    if let Some(test_notif) = notification.test_notification {
        info!("Test notification. version: {}", test_notif.version)
    }

    if let Some(otp_notif) = notification.one_time_product_notification {
        return Err(internal!("Received a one time product notification although there are no registered one time products. one_time_product_notification: {:?}", otp_notif));
    }

    Ok(())
}
