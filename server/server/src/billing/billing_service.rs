use crate::account_service::GetUsageHelperError;
use crate::billing::app_store_model::{NotificationChange, Subtype};
use crate::billing::app_store_service::verify_details;
use crate::billing::billing_model::{AppStoreUserInfo, BillingPlatform};
use crate::billing::billing_service::LockBillingWorkflowError::{
    ExistingRequestPending, UserNotFound,
};
use crate::billing::google_play_model::NotificationType;
use crate::billing::{
    app_store_service, google_play_client, google_play_service, stripe_client, stripe_service,
};
use crate::schema::Account;
use crate::ServerError::ClientError;
use crate::{
    account_service, RequestContext, ServerError, ServerState, FREE_TIER_USAGE_SIZE,
    PREMIUM_TIER_USAGE_SIZE,
};
use base64::DecodeError;
use hmdb::transaction::Transaction;
use libsecp256k1::PublicKey;
use lockbook_shared::api::{
    AppStoreAccountState, CancelSubscriptionError, CancelSubscriptionRequest,
    CancelSubscriptionResponse, GetSubscriptionInfoError, GetSubscriptionInfoRequest,
    GetSubscriptionInfoResponse, GooglePlayAccountState, PaymentPlatform, SubscriptionInfo,
    UpgradeAccountAppStoreError, UpgradeAccountAppStoreRequest, UpgradeAccountAppStoreResponse,
    UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
    UpgradeAccountGooglePlayResponse, UpgradeAccountStripeError, UpgradeAccountStripeRequest,
    UpgradeAccountStripeResponse,
};
use lockbook_shared::clock::get_time;
use lockbook_shared::file_metadata::Owner;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::*;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

#[derive(Debug)]
pub enum LockBillingWorkflowError {
    UserNotFound,
    ExistingRequestPending,
}

fn lock_subscription_profile(
    state: &ServerState, public_key: &PublicKey,
) -> Result<Account, ServerError<LockBillingWorkflowError>> {
    let owner = Owner(*public_key);
    state.index_db.transaction(|tx| {
        let mut account = tx
            .accounts
            .get(&owner)
            .ok_or(ClientError(UserNotFound))?
            .clone();
        let current_time = get_time().0 as u64;

        if current_time - account.billing_info.last_in_payment_flow < state.config.billing.millis_between_user_payment_flows {
            warn!(?owner, "User/Webhook is already in payment flow, or not enough time that has elapsed since a failed attempt");

            return Err(ClientError(ExistingRequestPending));
        }

        account.billing_info.last_in_payment_flow = current_time;
        tx.accounts.insert(owner, account.clone());

        debug!(?owner, "User successfully entered payment flow");

        Ok(account)
    })?
}

fn release_subscription_profile<T: Debug>(
    server_state: &ServerState, public_key: &PublicKey, account: Account,
) -> Result<(), ServerError<T>> {
    let mut account = account;
    Ok(server_state.index_db.transaction(|tx| {
        account.billing_info.last_in_payment_flow = 0;
        tx.accounts.insert(Owner(*public_key), account);
    })?)
}

pub async fn upgrade_account_app_store(
    context: RequestContext<'_, UpgradeAccountAppStoreRequest>,
) -> Result<UpgradeAccountAppStoreResponse, ServerError<UpgradeAccountAppStoreError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut account = lock_subscription_profile(server_state, &context.public_key)?;

    if account.billing_info.data_cap() == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountAppStoreError::AlreadyPremium));
    }

    debug!("Upgrading the account of a user through app store billing");

    if let Some(owner) = server_state
        .index_db
        .app_store_ids
        .get(&request.app_account_token)?
    {
        if owner.0 != context.public_key {
            return Err(ClientError(UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked));
        }
    }

    let expires = verify_details(
        &server_state.app_store_client,
        &server_state.config.billing.apple,
        &request.app_account_token,
        &request.original_transaction_id,
    )
    .await?;

    debug!("Successfully verified app store subscription");

    account.billing_info.billing_platform = Some(BillingPlatform::AppStore(AppStoreUserInfo {
        original_transaction_id: request.original_transaction_id.clone(),
        subscription_product_id: server_state
            .config
            .billing
            .apple
            .subscription_product_id
            .clone(),
        expiration_time: expires,
        account_state: AppStoreAccountState::Ok,
    }));

    server_state
        .index_db
        .app_store_ids
        .insert(request.app_account_token.clone(), Owner(context.public_key))?;

    release_subscription_profile::<UpgradeAccountAppStoreError>(
        server_state,
        &context.public_key,
        account,
    )?;

    Ok(UpgradeAccountAppStoreResponse {})
}

pub async fn upgrade_account_google_play(
    context: RequestContext<'_, UpgradeAccountGooglePlayRequest>,
) -> Result<UpgradeAccountGooglePlayResponse, ServerError<UpgradeAccountGooglePlayError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut account = lock_subscription_profile(server_state, &context.public_key)?;

    if account.billing_info.data_cap() == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountGooglePlayError::AlreadyPremium));
    }

    debug!("Upgrading the account of a user through google play billing");

    google_play_client::acknowledge_subscription(
        &server_state.config,
        &server_state.google_play_client,
        &request.purchase_token,
    )
    .await?;

    debug!("Acknowledged a user's google play subscription");

    let expiry_info = google_play_client::get_subscription(
        &server_state.config,
        &server_state.google_play_client,
        &request.purchase_token,
    )
    .await?;

    account.billing_info.billing_platform = Some(BillingPlatform::new_play_sub(
        &server_state.config,
        &request.purchase_token,
        expiry_info,
    )?);

    server_state
        .index_db
        .google_play_ids
        .insert(request.account_id.clone(), Owner(context.public_key))?;

    release_subscription_profile::<UpgradeAccountGooglePlayError>(
        server_state,
        &context.public_key,
        account,
    )?;

    debug!("Successfully upgraded a user through a google play subscription. public_key");

    Ok(UpgradeAccountGooglePlayResponse {})
}

pub async fn upgrade_account_stripe(
    context: RequestContext<'_, UpgradeAccountStripeRequest>,
) -> Result<UpgradeAccountStripeResponse, ServerError<UpgradeAccountStripeError>> {
    let (request, server_state) = (&context.request, context.server_state);

    debug!("Attempting to upgrade the account tier of to premium");

    let mut account = lock_subscription_profile(server_state, &context.public_key)?;

    if account.billing_info.data_cap() == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountStripeError::AlreadyPremium));
    }

    let maybe_user_info = account
        .billing_info
        .billing_platform
        .and_then(|info| match info {
            BillingPlatform::Stripe(stripe_info) => Some(stripe_info),
            _ => None,
        });

    let user_info = stripe_service::create_subscription(
        server_state,
        &context.public_key,
        &request.account_tier,
        maybe_user_info,
    )
    .await?;

    account.billing_info.billing_platform = Some(BillingPlatform::Stripe(user_info));
    release_subscription_profile::<UpgradeAccountStripeError>(
        server_state,
        &context.public_key,
        account,
    )?;

    debug!("Successfully upgraded the account tier of from free to premium");

    Ok(UpgradeAccountStripeResponse {})
}

pub async fn get_subscription_info(
    context: RequestContext<'_, GetSubscriptionInfoRequest>,
) -> Result<GetSubscriptionInfoResponse, ServerError<GetSubscriptionInfoError>> {
    let account = context
        .server_state
        .index_db
        .accounts
        .get(&Owner(context.public_key))?
        .ok_or(ClientError(GetSubscriptionInfoError::UserNotFound))?;

    let subscription_info = account
        .billing_info
        .billing_platform
        .map(|info| match info {
            BillingPlatform::Stripe(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::Stripe {
                    card_last_4_digits: info.last_4.clone(),
                },
                period_end: info.expiration_time,
            },
            BillingPlatform::GooglePlay(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::GooglePlay {
                    account_state: info.account_state.clone(),
                },
                period_end: info.expiration_time,
            },
            BillingPlatform::AppStore(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::AppStore {
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
    let mut account = lock_subscription_profile(server_state, &context.public_key)?;

    if account.billing_info.data_cap() == FREE_TIER_USAGE_SIZE {
        return Err(ClientError(CancelSubscriptionError::NotPremium));
    }

    let usage: u64 = server_state
        .index_db
        .transaction(|tx| account_service::get_usage_helper(tx, &context.public_key))?
        .map_err(|e| match e {
            GetUsageHelperError::UserNotFound => ClientError(CancelSubscriptionError::NotPremium),
        })?
        .iter()
        .map(|a| a.size_bytes)
        .sum();

    if usage > FREE_TIER_USAGE_SIZE {
        debug!("Cannot downgrade user to free since they are over the data cap");
        return Err(ClientError(CancelSubscriptionError::UsageIsOverFreeTierDataCap));
    }

    match account.billing_info.billing_platform {
        None => return Err(internal!("A user somehow has premium tier usage, but no billing information on redis. public_key: {:?}", context.public_key)),
        Some(BillingPlatform::GooglePlay(ref mut info)) => {
            debug!("Canceling google play subscription of user");

            if let GooglePlayAccountState::Canceled = &info.account_state {
                return Err(ClientError(CancelSubscriptionError::AlreadyCanceled))
            }

            google_play_client::cancel_subscription(
                &server_state.config,
                &server_state.google_play_client,
                &info.purchase_token,
            ).await?;

            info.account_state = GooglePlayAccountState::Canceled;
            debug!("Successfully canceled google play subscription of user");
        }
        Some(BillingPlatform::Stripe(ref info)) => {
            debug!("Canceling stripe subscription of user");

            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &info.subscription_id.parse()?,
            )
                .await
                .map_err::<ServerError<CancelSubscriptionError>, _>(|err| internal!("{:?}", err))?;

            account.billing_info.billing_platform = None;

            debug!("Successfully canceled stripe subscription");
        }
        Some(BillingPlatform::AppStore(_)) => {
            return Err(ClientError(CancelSubscriptionError::CannotCancelForAppStore));
        }
    }

    release_subscription_profile::<CancelSubscriptionError>(
        server_state,
        &context.public_key,
        account,
    )?;

    Ok(CancelSubscriptionResponse {})
}

async fn save_subscription_profile<T: Debug, F: Fn(&mut Account) -> Result<(), ServerError<T>>>(
    state: &ServerState, public_key: &PublicKey, update_subscription_profile: F,
) -> Result<(), ServerError<T>> {
    let millis_between_lock_attempts = state.config.billing.time_between_lock_attempts;
    loop {
        match lock_subscription_profile(state, public_key) {
            Ok(ref mut sub_profile) => {
                update_subscription_profile(sub_profile)?;
                release_subscription_profile(state, public_key, sub_profile.clone())?;
                break;
            }
            Err(ClientError(ExistingRequestPending)) => {
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

    let event_type = event.event_type;
    debug!(?event_type, "Verified stripe request");

    match (&event.event_type, &event.data.object) {
        (stripe::EventType::InvoicePaymentFailed, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = stripe_service::get_public_key(server_state, invoice)?;
                let owner = Owner(public_key);

                debug!(
                    ?owner,
                    "User's tier is being reduced due to failed renewal payment in stripe"
                );

                save_subscription_profile(server_state, &public_key, |account| {
                    account.billing_info.billing_platform = None;
                    Ok(())
                })
                .await?;
            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = stripe_service::get_public_key(server_state, invoice)?;
                let owner = Owner(public_key);

                debug!(
                    ?owner,
                    "User's subscription period_end is being changed after successful renewal",
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

                save_subscription_profile(server_state, &public_key, |account| {
                    if let Some(BillingPlatform::Stripe(ref mut info)) =
                        account.billing_info.billing_platform
                    {
                        info.expiration_time = subscription_period_end as u64;
                    }
                    Ok(())
                })
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

    if let Some(sub_notif) = notification.subscription_notification {
        debug!(?sub_notif, "Notification is for a subscription");

        let subscription = google_play_client::get_subscription(
            &server_state.config,
            &server_state.google_play_client,
            &sub_notif.purchase_token,
        )
        .await
        .map_err(|e| internal!("{:#?}", e))?;

        let notification_type = sub_notif.notification_type();
        if let NotificationType::SubscriptionPurchased = notification_type {
            return Ok(());
        }

        let public_key = google_play_service::get_public_key(
            server_state,
            &sub_notif,
            &subscription,
            &notification_type,
        )?;
        let owner = Owner(public_key);

        debug!(
            ?owner,
            ?notification_type,
            "Updating google play user's subscription profile to match new subscription state"
        );

        save_subscription_profile(server_state, &public_key, |account| {
            if let Some(BillingPlatform::GooglePlay(ref mut info)) =
                account.billing_info.billing_platform
            {
                match notification_type {
                    NotificationType::SubscriptionRecovered
                    | NotificationType::SubscriptionRestarted
                    | NotificationType::SubscriptionRenewed => {
                        info.account_state = GooglePlayAccountState::Ok;
                        info.expiration_time = google_play_service::get_subscription_period_end(
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
                        info.expiration_time = google_play_service::get_subscription_period_end(
                            &subscription,
                            &notification_type,
                            public_key,
                        )?;
                    }
                    NotificationType::SubscriptionExpired
                    | NotificationType::SubscriptionRevoked => {
                        if info.purchase_token == sub_notif.purchase_token {
                            account.billing_info.billing_platform = None
                        } else {
                            let old_purchase_token = &sub_notif.purchase_token;
                            let new_purchase_token = &info.purchase_token;
                            debug!(
                                ?old_purchase_token,
                                ?new_purchase_token,
                                "Expired or revoked subscription was tied to an old purchase_token"
                            );
                        }
                    }
                    NotificationType::SubscriptionCanceled => {
                        info.account_state = GooglePlayAccountState::Canceled;
                        let cancellation_reason = &subscription.cancel_survey_result;
                        let owner = Owner(public_key);
                        debug!(?cancellation_reason, ?owner, "Subscription cancelled");
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
                Err(internal!("Cannot get any billing info for user. public_key: {:?}", public_key))
            }
        })
        .await?;
    }

    if let Some(test_notif) = notification.test_notification {
        let version = &test_notif.version;
        debug!(?version, "Test notification");
    }

    if let Some(otp_notif) = notification.one_time_product_notification {
        return Err(internal!("Received a one time product notification although there are no registered one time products. one_time_product_notification: {:?}", otp_notif));
    }

    Ok(())
}

#[derive(Debug)]
pub enum AppStoreNotificationError {
    InvalidJWS,
}

pub async fn app_store_notification_webhook(
    server_state: &Arc<ServerState>, body: Bytes,
) -> Result<(), ServerError<AppStoreNotificationError>> {
    let resp =
        app_store_service::decode_verify_notification(&server_state.config.billing.apple, &body)?;

    if let NotificationChange::Subscribed = resp.notification_type {
        return Ok(());
    } else if let NotificationChange::Test = resp.notification_type {
        debug!(?resp, "This is a test notification.");
        return Ok(());
    }

    let trans = app_store_service::decode_verify_transaction(
        &server_state.config.billing.apple,
        &resp.clone().data.encoded_transaction_info.unwrap(),
    )?;
    let public_key = app_store_service::get_public_key(server_state, &trans)?;

    let owner = Owner(public_key);

    debug!(
        ?owner,
        ?resp.notification_type,
        ?resp.subtype,
        "Updating app store user's subscription profile to match new subscription state"
    );

    save_subscription_profile(server_state, &public_key, |account| {
        if let Some(BillingPlatform::AppStore(ref mut info)) = account.billing_info.billing_platform
        {
            match resp.notification_type {
                NotificationChange::DidFailToRenew => {
                    info.account_state = if let Some(Subtype::GracePeriod) = resp.subtype {
                        AppStoreAccountState::GracePeriod
                    } else {
                        AppStoreAccountState::FailedToRenew
                    };
                }
                NotificationChange::Expired => match resp.subtype {
                    Some(Subtype::BillingRetry) => {
                        info!(?owner, ?resp, "Subscription failed to renew due to billing issues.");
                    }
                    Some(Subtype::Voluntary) => {
                        info!(?owner, ?resp, "Subscription cancelled");
                    }
                    _ => {
                        return Err(internal!(
                            "Unexpected subtype: {:?}, notification_type {:?}, public_key: {:?}",
                            resp.subtype,
                            resp.notification_type,
                            public_key
                        ))
                    }
                },
                NotificationChange::GracePeriodExpired => {
                    account.billing_info.billing_platform = None;
                }
                NotificationChange::Refund => {
                    info!(?resp, "A user has requested a refund.");
                }
                NotificationChange::RefundDeclined => {
                    info!(?resp, "A user's refund request has been denied.");
                }
                NotificationChange::DidChangeRenewalStatus => match resp.subtype {
                    Some(Subtype::AutoRenewEnabled) => {
                        info.account_state = AppStoreAccountState::Ok;
                        info.expiration_time = trans.expires_date;
                    }
                    Some(Subtype::AutoRenewDisabled) => {
                        account.billing_info.billing_platform = None;
                    }
                    _ => {
                        return Err(internal!(
                            "Unexpected subtype: {:?}, notification_type {:?}, public_key: {:?}",
                            resp.subtype,
                            resp.notification_type,
                            public_key
                        ))
                    }
                },
                NotificationChange::RenewalExtended => info.expiration_time = trans.expires_date,
                NotificationChange::DidRenew => {
                    if let Some(Subtype::BillingRecovery) = resp.subtype {
                        info.account_state = AppStoreAccountState::Ok;
                    }
                    info.expiration_time = trans.expires_date
                }
                NotificationChange::ConsumptionRequest
                | NotificationChange::Subscribed
                | NotificationChange::DidChangeRenewalPref
                | NotificationChange::OfferRedeemed
                | NotificationChange::PriceIncrease
                | NotificationChange::Revoke
                | NotificationChange::Test => {
                    return Err(internal!(
                        "Unexpected notification change: {:?} {:?}, public_key: {:?}",
                        resp.notification_type,
                        resp.subtype,
                        public_key
                    ))
                }
            }

            Ok(())
        } else {
            Err(internal!("Cannot get any billing info for user. public_key: {:?}", public_key))
        }
    })
    .await?;

    Ok(())
}

pub fn stringify_public_key(pk: &PublicKey) -> String {
    base64::encode(pk.serialize_compressed())
}
