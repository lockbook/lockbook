use crate::account_service::GetUsageHelperError;
use crate::billing::billing_model::{
    BillingPlatform, GooglePlayUserInfo, StripeUserInfo, SubscriptionProfile,
};
use crate::billing::google_play_model::{
    DeveloperNotification, NotificationType, PubSubNotification,
};
use crate::billing::{google_play_client, stripe_client};
use crate::keys::public_key_from_stripe_customer_id;
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, keys, RequestContext, ServerError, ServerState, FREE_TIER_USAGE_SIZE,
    PREMIUM_TIER_USAGE_SIZE,
};
use base64::DecodeError;
use deadpool_redis::Connection;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, CancelSubscriptionResponse,
    GetSubscriptionInfoError, GetSubscriptionInfoRequest, GetSubscriptionInfoResponse,
    GooglePlayAccountState, PaymentMethod, PaymentPlatform, StripeAccountTier, SubscriptionInfo,
    UpgradeAccountAndroidError, UpgradeAccountAndroidRequest, UpgradeAccountAndroidResponse,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest, UpgradeAccountStripeResponse,
};
use log::{info, warn};
use redis_utils::converters::{JsonGet, JsonSet, PipelineJsonSet};
use redis_utils::tx;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use stripe::Invoice;
use uuid::Uuid;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

#[derive(Debug)]
pub enum LockBillingWorkflowError {
    ExistingRequestPending,
}

async fn lock_billing_workflow(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection, millis_between_payment_flows: u64,
) -> Result<SubscriptionProfile, ServerError<LockBillingWorkflowError>> {
    let mut sub_hist = SubscriptionProfile::default();

    let tx_result = tx!(con, pipe, &[keys::subscription_profile(public_key)], {
        sub_hist = con
            .maybe_json_get(keys::subscription_profile(public_key))
            .await?
            .unwrap_or_default();

        let current_time = get_time().0 as u64;

        if current_time - sub_hist.last_in_payment_flow < millis_between_payment_flows {
            warn!(
                "User is already in payment flow, or this not enough time has elapsed since a failed attempt. public_key: {}",
                keys::stringify_public_key(public_key)
            );

            return Err(Abort(ClientError(LockBillingWorkflowError::ExistingRequestPending)));
        }

        sub_hist.last_in_payment_flow = current_time;

        pipe.json_set(keys::subscription_profile(public_key), &sub_hist)
    });
    return_if_error!(tx_result);

    info!(
        "User successfully entered payment flow. public_key: {}",
        keys::stringify_public_key(public_key)
    );

    Ok(sub_hist)
}

async fn reset_billing_lock<E: Debug>(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection,
    sub_hist: &mut SubscriptionProfile,
) -> Result<(), ServerError<E>> {
    sub_hist.last_in_payment_flow = 0;
    Ok(con
        .json_set(keys::subscription_profile(public_key), &sub_hist)
        .await?)
}

pub async fn get_subscription_info(
    context: RequestContext<'_, GetSubscriptionInfoRequest>,
) -> Result<GetSubscriptionInfoResponse, ServerError<GetSubscriptionInfoError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let sub_hist: SubscriptionProfile = match con
        .maybe_json_get(keys::subscription_profile(&context.public_key))
        .await?
    {
        Some(sub_hist) => sub_hist,
        None => return Ok(GetSubscriptionInfoResponse { subscription_info: None }),
    };

    let subscription_info = match (sub_hist.data_cap, sub_hist.info) {
        (FREE_TIER_USAGE_SIZE, _) => None,
        (PREMIUM_TIER_USAGE_SIZE, Some(BillingPlatform::Stripe(info))) => Some(SubscriptionInfo {
            payment_platform: PaymentPlatform::Stripe { card_last_4_digits: info.last_4.clone() },
            period_end: info.expiration_time,
        }),
        (PREMIUM_TIER_USAGE_SIZE, Some(BillingPlatform::GooglePlay(info))) => {
            Some(SubscriptionInfo {
                payment_platform: PaymentPlatform::GooglePlay {
                    account_state: info.account_state.clone(),
                },
                period_end: info.expiration_time,
            })
        }
        (_, _) => {
            return Err(internal!(
                "No billing information despite being a premium tier. public_key: {:?}",
                context.public_key
            ))
        }
    };

    Ok(GetSubscriptionInfoResponse { subscription_info })
}

pub async fn upgrade_account_android(
    context: RequestContext<'_, UpgradeAccountAndroidRequest>,
) -> Result<UpgradeAccountAndroidResponse, ServerError<UpgradeAccountAndroidError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_hist = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(UpgradeAccountAndroidError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_hist.data_cap == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountAndroidError::AlreadyPremium));
    }

    info!(
        "Upgrading the account of a user through google play billing. public_key: {:?}.",
        context.public_key
    );

    google_play_client::acknowledge_subscription(
        &server_state.android_publisher,
        &server_state.config.google.premium_subscription_product_id,
        &request.purchase_token,
    )
    .await?;

    info!("Acknowledged a user's google play subscription.");

    let subscription = google_play_client::get_subscription(
        &server_state.android_publisher,
        &server_state.config.google.premium_subscription_product_id,
        &request.purchase_token,
    )
    .await?;

    sub_hist.info = Some(BillingPlatform::GooglePlay(GooglePlayUserInfo {
        purchase_token: request.purchase_token.clone(),
        subscription_product_id: server_state
            .config
            .google
            .premium_subscription_product_id
            .clone(),
        subscription_offer_id: server_state
            .config
            .google
            .premium_subscription_offer_id
            .clone(),
        expiration_time: subscription
            .borrow()
            .expiry_time_millis
            .as_ref()
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

    sub_hist.data_cap = PREMIUM_TIER_USAGE_SIZE;
    reset_billing_lock(&context.public_key, &mut con, &mut sub_hist).await?;
    con.json_set(keys::public_key_from_gp_account_id(&request.account_id), context.public_key)
        .await?;

    info!(
        "Successfully upgraded a user through a google play subscription. public_key: {:?}",
        context.public_key
    );

    Ok(UpgradeAccountAndroidResponse {})
}

pub async fn cancel_subscription(
    context: RequestContext<'_, CancelSubscriptionRequest>,
) -> Result<CancelSubscriptionResponse, ServerError<CancelSubscriptionError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_hist = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(CancelSubscriptionError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_hist.data_cap == FREE_TIER_USAGE_SIZE {
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

    match sub_hist.info {
        None => return Err(internal!("A user somehow has premium tier usage, but no billing information on redis. public_key: {:?}", context.public_key)),
        Some(BillingPlatform::GooglePlay(ref mut info)) => {
            info!("Canceling google play subscription of user. public_key: {:?}.", context.public_key);

            google_play_client::cancel_subscription(
                &server_state.android_publisher,
                &info.subscription_product_id,
                &info.purchase_token,
            ).await?;

            info.account_state = GooglePlayAccountState::Canceled;
            info!("Successfully canceled google play subscription of user. public_key: {:?}.", context.public_key);

            // You do not get rid of the data cap until they have used up the rest of their time.
        }
        Some(BillingPlatform::Stripe(ref info)) => {
            info!("Canceling stripe subscription of user. public_key: {:?}.", context.public_key);

            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &stripe::SubscriptionId::from_str(&info.subscription_id)?,
            )
                .await
                .map_err(|err| internal!("{:?}", err))?;

            sub_hist.data_cap = FREE_TIER_USAGE_SIZE;
            info!("Successfully canceled stripe subscription. public_key: {:?}", context.public_key);

        }
    }

    reset_billing_lock(&context.public_key, &mut con, &mut sub_hist).await?;

    Ok(CancelSubscriptionResponse {})
}

pub async fn upgrade_account_stripe(
    context: RequestContext<'_, UpgradeAccountStripeRequest>,
) -> Result<UpgradeAccountStripeResponse, ServerError<UpgradeAccountStripeError>> {
    let (request, server_state) = (&context.request, context.server_state);

    info!("Attempting to switch account tier of {:?} to premium", context.public_key);

    let mut con = server_state.index_db_pool.get().await?;

    let mut sub_hist = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
            ClientError(UpgradeAccountStripeError::ExistingRequestPending)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    if sub_hist.data_cap == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountStripeError::AlreadyPremium));
    }

    let maybe_user_info = sub_hist.info.and_then(|info| {
        if let BillingPlatform::Stripe(stripe_info) = info {
            Some(stripe_info)
        } else {
            None
        }
    });

    let (user_info, new_data_cap) = create_subscription(
        server_state,
        &mut con,
        &context.public_key,
        &request.account_tier,
        maybe_user_info,
    )
    .await?;

    sub_hist.info = Some(BillingPlatform::Stripe(user_info));
    sub_hist.data_cap = new_data_cap;

    reset_billing_lock(&context.public_key, &mut con, &mut sub_hist).await?;

    info!(
        "Successfully switched the account tier of {:?} from free to premium.",
        context.public_key
    );

    Ok(UpgradeAccountStripeResponse {})
}

async fn create_subscription(
    server_state: &ServerState, con: &mut deadpool_redis::Connection, public_key: &PublicKey,
    account_tier: &StripeAccountTier, maybe_user_info: Option<StripeUserInfo>,
) -> Result<(StripeUserInfo, u64), ServerError<UpgradeAccountStripeError>> {
    let (payment_method, data_cap) = match account_tier {
        StripeAccountTier::Premium(payment_method) => (payment_method, PREMIUM_TIER_USAGE_SIZE),
    };

    let (customer_id, customer_name, payment_method_id, last_4) = match payment_method {
        PaymentMethod::NewCard { number, exp_year, exp_month, cvc } => {
            info!("Creating a new card for public_key: {:?}", public_key);
            let payment_method_resp = stripe_client::create_payment_method(
                &server_state.stripe_client,
                number,
                *exp_month,
                *exp_year,
                cvc,
            )
            .await?;

            let last_4 = payment_method_resp
                .card
                .as_ref()
                .ok_or_else(|| {
                    internal!(
                        "Cannot retrieve card info from payment method response: {:?}",
                        payment_method_resp
                    )
                })?
                .last4
                .clone();

            info!("Created a new payment method. last_4: {}, public_key: {:?}", last_4, public_key);

            let (customer_id, customer_name) = match &maybe_user_info {
                None => {
                    info!(
                        "User has no customer_id. Creating one with stripe now. public_key: {}",
                        keys::stringify_public_key(public_key)
                    );

                    let customer_name = Uuid::new_v4();
                    let customer_resp = stripe_client::create_customer(
                        &server_state.stripe_client,
                        &customer_name.to_string(),
                        payment_method_resp.id.clone(),
                    )
                    .await?;
                    let customer_id = customer_resp.id.to_string();

                    info!("Created customer_id: {}. public_key: {:?}", customer_id, public_key);

                    con.json_set(public_key_from_stripe_customer_id(&customer_id), public_key)
                        .await?;

                    (customer_resp.id, customer_name)
                }
                Some(user_info) => {
                    info!(
                        "User already has customer_id: {} public_key: {:?}",
                        user_info.customer_id, public_key
                    );

                    let customer_id = stripe::CustomerId::from_str(&user_info.customer_id)?;

                    info!(
                        "Disabling old card since a new card has just been added. public_key: {:?}",
                        public_key
                    );

                    stripe_client::detach_payment_method_from_customer(
                        &server_state.stripe_client,
                        &stripe::PaymentMethodId::from_str(&user_info.payment_method_id)?,
                    )
                    .await?;

                    (customer_id, user_info.customer_name)
                }
            };

            info!(
                "Creating a setup intent to confirm a users payment method for their subscription. public_key: {:?}",
                public_key
            );

            let setup_intent_resp = stripe_client::create_setup_intent(
                &server_state.stripe_client,
                customer_id.clone(),
                payment_method_resp.id.clone(),
            )
            .await?;

            info!(
                "Created a setup intent: {}, public_key: {:?}",
                setup_intent_resp.id.to_string(),
                public_key
            );

            (customer_id, customer_name, payment_method_resp.id.to_string(), last_4)
        }
        PaymentMethod::OldCard => {
            info!("Using an old card stored on redis for public_key: {:?}", public_key);

            let user_info = maybe_user_info
                .ok_or(ClientError(UpgradeAccountStripeError::OldCardDoesNotExist))?;

            (
                stripe::CustomerId::from_str(&user_info.customer_id)?,
                user_info.customer_name,
                user_info.payment_method_id,
                user_info.last_4,
            )
        }
    };

    info!("Successfully retrieved card for public_key: {:?}", public_key);

    let subscription_resp =
        stripe_client::create_subscription(server_state, customer_id.clone(), &payment_method_id)
            .await?;

    info!(
        "Successfully create subscription: {}, public_key: {:?}",
        subscription_resp.id, public_key
    );

    Ok((
        StripeUserInfo {
            customer_id: customer_id.to_string(),
            customer_name,
            payment_method_id: payment_method_id.to_string(),
            last_4,
            subscription_id: subscription_resp.id.to_string(),
            expiration_time: subscription_resp.current_period_end as u64,
        },
        data_cap,
    ))
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
    let payload = std::str::from_utf8(&request_body).map_err(|e| {
        ClientError(StripeWebhookError::InvalidBody(format!("Cannot get body as str: {:?}", e)))
    })?;
    let sig = stripe_sig.to_str().map_err(|e| {
        ClientError(StripeWebhookError::InvalidHeader(format!("Cannot get header as str: {:?}", e)))
    })?;

    info!("Verifying a stripe webhook request.");

    let event =
        stripe::Webhook::construct_event(payload, sig, &server_state.config.stripe.signing_secret)?;

    info!("Verified stripe request. event: {:?}.", event.event_type);

    let mut con = server_state.index_db_pool.get().await?;

    match (&event.event_type, &event.data.object) {
        (stripe::EventType::InvoicePaymentFailed, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = get_public_key(&mut con, invoice).await?;

                info!(
                    "User tier being reduced due to failed renewal payment via stripe. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );

                let tx_result = tx!(&mut con, pipe, &[keys::subscription_profile(&public_key)], {
                    match lock_billing_workflow(
                        &public_key,
                        &mut con,
                        server_state.config.stripe.millis_between_user_payment_flows,
                    )
                    .await
                    {
                        Ok(mut sub_hist) => {
                            sub_hist.data_cap = FREE_TIER_USAGE_SIZE;
                            sub_hist.info = None;

                            pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                        }
                        Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => {
                            Ok(&mut pipe)
                        }
                        Err(err) => Err(Abort(internal!(
                            "Cannot get billing lock in stripe webhooks: {:#?}",
                            err
                        ))),
                    }
                });
                return_if_error!(tx_result);
            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) = invoice.billing_reason {
                let public_key = get_public_key(&mut con, invoice).await?;

                let subscription_period_end = match &invoice.subscription {
                    None => {
                        return Err(internal!(
                            "There should be a subscription tied to this invoice: {:?}",
                            invoice
                        ));
                    }
                    Some(stripe::Expandable::Id(_)) => {
                        return Err(internal!(
                            "The subscription should be expanded in this invoice: {:?}",
                            invoice
                        ));
                    }
                    Some(stripe::Expandable::Object(subscription)) => {
                        subscription.current_period_end
                    }
                };

                let tx_result = tx!(&mut con, pipe, &[keys::subscription_profile(&public_key)], {
                    match lock_billing_workflow(
                        &public_key,
                        &mut con,
                        server_state.config.stripe.millis_between_user_payment_flows,
                    )
                    .await
                    {
                        Ok(ref mut sub_hist) => {
                            if let Some(BillingPlatform::Stripe(ref mut info)) = sub_hist.info {
                                info.expiration_time = subscription_period_end as u64;
                            }

                            sub_hist.last_in_payment_flow = 0;
                            pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                        }
                        Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => {
                            Ok(&mut pipe)
                        }
                        Err(err) => Err(Abort(internal!(
                            "Cannot get billing lock in stripe webhooks: {:#?}",
                            err
                        ))),
                    }
                });
                return_if_error!(tx_result);

                info!(
                    "User's subscription period_end is changed after successful renewal. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );
            }
        }
        (_, _) => {
            return Err(internal!("Unexpected and unhandled stripe event: {:?}", event.event_type));
        }
    }

    Ok(())
}

async fn get_public_key(
    con: &mut Connection, invoice: &Invoice,
) -> Result<PublicKey, ServerError<StripeWebhookError>> {
    let customer_id = match invoice
        .customer
        .as_ref()
        .ok_or_else(|| {
            ClientError(StripeWebhookError::InvalidBody(
                "Cannot retrieve the customer_id.".to_string(),
            ))
        })?
        .deref()
    {
        stripe::Expandable::Id(id) => id.to_string(),
        stripe::Expandable::Object(customer) => customer.id.to_string(),
    };

    let public_key: PublicKey = con
        .maybe_json_get(public_key_from_stripe_customer_id(&customer_id))
        .await?
        .ok_or_else(|| {
            internal!("There is no public_key related to this customer_id: {:?}", customer_id)
        })?;

    Ok(public_key)
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
    if !constant_time_eq::constant_time_eq(
        query_parameters
            .get("token")
            .ok_or(ClientError(GooglePlayWebhookError::InvalidToken))?
            .as_bytes(),
        server_state.config.google.pubsub_token.as_bytes(),
    ) {
        return Err(ClientError(GooglePlayWebhookError::InvalidToken));
    }

    info!("Parsing pubsub notification and extracting the developer notification.");

    let pubsub_notif = serde_json::from_slice::<PubSubNotification>(&request_body)?;
    let data = base64::decode(pubsub_notif.message.data)
        .map_err(|e| ClientError(GooglePlayWebhookError::CannotDecodePubSubData(e)))?;

    let notification = serde_json::from_slice::<DeveloperNotification>(&data)?;

    let mut con = server_state.index_db_pool.get().await?;

    if let Some(sub_notif) = notification.subscription_notification {
        info!("Notification is for a subscription: {:?}", sub_notif);

        let subscription = google_play_client::get_subscription(
            &server_state.android_publisher,
            &sub_notif.subscription_id,
            &sub_notif.purchase_token,
        )
        .await
        .map_err(|e| internal!("{:#?}", e))?;

        let notification_type = sub_notif.notification_type();
        if let NotificationType::SubscriptionPurchased = notification_type {
            return Ok(());
        }

        let account_id = &subscription
            .obfuscated_external_account_id
            .clone()
            .ok_or_else(|| {
                internal!("There should be an account id attached to a purchase: {:?}", sub_notif)
            })?;

        info!("Retrieved full subscription info for notification event {:?} with an obfuscated id of {:?}", notification_type, account_id);

        let public_key: PublicKey = con
            .maybe_json_get(keys::public_key_from_gp_account_id(account_id))
            .await?
            .ok_or_else(|| {
                internal!("There is no public_key related to this account_id: {:?}", account_id)
            })?;

        info!("Updating subscription history to match new subscription state.");

        let tx_result = tx!(&mut con, pipe, &[keys::subscription_profile(&public_key)], {
            match lock_billing_workflow(
                &public_key,
                &mut con,
                server_state.config.stripe.millis_between_user_payment_flows,
            )
            .await
            {
                Ok(mut sub_hist) => {
                    if let Some(BillingPlatform::GooglePlay(ref mut info)) = sub_hist.info {
                        match notification_type {
                            NotificationType::SubscriptionRecovered => {
                                info.account_state = GooglePlayAccountState::Ok;
                                info.expiration_time = subscription.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                sub_hist.data_cap = PREMIUM_TIER_USAGE_SIZE;

                                sub_hist.last_in_payment_flow = 0;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                                // give back premium and update time exp
                            }
                            NotificationType::SubscriptionRenewed
                            | NotificationType::SubscriptionRestarted => {
                                info.account_state = GooglePlayAccountState::Ok;
                                info.expiration_time = subscription.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;

                                sub_hist.last_in_payment_flow = 0;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                                // change expiry time
                            }
                            NotificationType::SubscriptionInGracePeriod => {
                                info.account_state = GooglePlayAccountState::GracePeriod;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                            }
                            NotificationType::SubscriptionOnHold => {
                                info.account_state = GooglePlayAccountState::OnHold;
                                info.expiration_time = subscription.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                sub_hist.data_cap = FREE_TIER_USAGE_SIZE;

                                sub_hist.last_in_payment_flow = 0;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                            }
                            NotificationType::SubscriptionExpired
                            | NotificationType::SubscriptionRevoked => {
                                info.expiration_time = subscription.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                sub_hist.data_cap = FREE_TIER_USAGE_SIZE;
                                sub_hist.info = None;

                                sub_hist.last_in_payment_flow = 0;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                                // remove premium data cap and update exp date to past (for hold)
                            }
                            NotificationType::SubscriptionCanceled => {
                                info.account_state = GooglePlayAccountState::Canceled;
                                info!(
                                    "Reason of cancellation reason of user: {:?}",
                                    subscription.cancel_survey_result
                                );

                                sub_hist.last_in_payment_flow = 0;
                                pipe.json_set(keys::subscription_profile(&public_key), &sub_hist)
                            }
                            NotificationType::SubscriptionPurchased => Ok(&mut pipe),
                            NotificationType::SubscriptionPriceChangeConfirmed
                            | NotificationType::SubscriptionDeferred
                            | NotificationType::SubscriptionPaused
                            | NotificationType::SubscriptionPausedScheduleChanged => {
                                info!(
                                    "Unexpected subscription notification: {:?}, public_key: {:?}",
                                    notification_type, public_key
                                );
                                Ok(&mut pipe)
                            }
                            NotificationType::Unknown => Err(Abort(internal!(
                                "Unknown subscription change. public_key: {:?}",
                                public_key
                            ))),
                        }
                    } else {
                        Err(Abort(internal!(
                            "Cannot get any billing info for user. public_key: {:?}",
                            public_key
                        )))
                    }
                }
                Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => Ok(&mut pipe),
                Err(err) => {
                    Err(Abort(internal!("Cannot get billing lock in stripe webhooks: {:#?}", err)))
                }
            }
        });
        return_if_error!(tx_result);
    }

    if let Some(test_notif) = notification.test_notification {
        info!("Test notification hit: {}", test_notif.version)
    }

    if let Some(otp_notif) = notification.one_time_product_notification {
        return Err(internal!("Received a one time product notification although there are no registered one time products. one_time_product_notification: {:?}", otp_notif));
    }

    Ok(())
}
