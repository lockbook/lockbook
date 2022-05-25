use crate::account_service::GetUsageHelperError;
use crate::billing::billing_model::{BillingInfo, BillingLock, GooglePlayUserInfo, StripeUserInfo};
use crate::billing::google_play_client::SimpleGCPError;
use crate::billing::google_play_model::{DeveloperNotification, NotificationType};
use crate::billing::{google_play_client, stripe_client};
use crate::keys::{data_cap, public_key_from_stripe_customer_id};
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, keys, RequestContext, ServerError, ServerState, FREE_TIER_USAGE_SIZE,
    PREMIUM_TIER_USAGE_SIZE,
};
use base64::DecodeError;
use deadpool_redis::redis::AsyncCommands;
use google_pubsub1::api::PubsubMessage;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, CancelSubscriptionResponse,
    ConfirmAndroidSubscriptionError, ConfirmAndroidSubscriptionRequest,
    ConfirmAndroidSubscriptionResponse, GetCreditCardError, GetCreditCardRequest,
    GetCreditCardResponse, GetSubscriptionInfoError, GetSubscriptionInfoRequest,
    GetSubscriptionInfoResponse, PaymentMethod, PaymentPlatform, StripeAccountTier,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest, UpgradeAccountStripeResponse,
};
use log::info;
use redis_utils::converters::{JsonGet, JsonSet, PipelineJsonSet};
use redis_utils::tx;
use std::fmt::Debug;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use deadpool_redis::Connection;
use stripe::Invoice;
use uuid::Uuid;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;
use std::borrow::Borrow;

#[derive(Debug)]
pub enum LockBillingWorkflowError {
    ConcurrentRequestsAreTooSoon,
}

async fn lock_billing_workflow(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection, millis_between_payment_flows: u64,
) -> Result<BillingLock, ServerError<LockBillingWorkflowError>> {
    let mut billing_lock = BillingLock::default();

    let tx_result = tx!(con, pipe, &[keys::billing_lock(public_key)], {
        billing_lock = con
            .maybe_json_get(keys::billing_lock(public_key))
            .await?
            .unwrap_or_default();

        let current_time = get_time().0 as u64;

        if current_time - billing_lock.last_in_payment_flow < millis_between_payment_flows {
            info!(
                "User is already in payment flow, or this request is too soon after a failed one. public_key: {}",
                keys::stringify_public_key(public_key)
            );

            return Err(Abort(ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon)));
        }

        billing_lock.last_in_payment_flow = current_time;

        pipe.json_set(keys::billing_lock(public_key), &billing_lock)
    });
    return_if_error!(tx_result);

    info!(
        "User successfully entered payment flow. public_key: {}",
        keys::stringify_public_key(public_key)
    );

    Ok(billing_lock)
}

async fn set_billing_lock<E: Debug>(public_key: &PublicKey, con: &mut deadpool_redis::Connection, billing_lock: &mut BillingLock) -> Result<(), ServerError<E>> {
    billing_lock.last_in_payment_flow = 0;
    Ok(con.json_set(keys::billing_lock(&public_key), &billing_lock).await?)
}



pub async fn get_subscription_info(
    context: RequestContext<'_, GetSubscriptionInfoRequest>,
) -> Result<GetSubscriptionInfoResponse, ServerError<GetSubscriptionInfoError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    if current_data_cap == FREE_TIER_USAGE_SIZE {
        return Err(ClientError(GetSubscriptionInfoError::NotPremium));
    }

    let billing_lock: BillingLock = con
        .maybe_json_get(keys::billing_lock(&context.public_key))
        .await?
        .ok_or_else(|| {
            internal!(
                "No billing lock despite being a premium tier. public_key: {:?}",
                context.public_key
            )
        })?;

    match billing_lock.info.last() {
        None => Err(internal!(
            "No billing information despite being a premium tier. public_key: {:?}",
            context.public_key
        )),
        Some(BillingInfo::Stripe(info)) => Ok(GetSubscriptionInfoResponse {
            payment_platform: PaymentPlatform::Stripe { card_last_4_digits: info.last_4.clone() },
            period_end: info.expiration_time,
        }),
        Some(BillingInfo::GooglePlay(info)) => Ok(GetSubscriptionInfoResponse {
            payment_platform: PaymentPlatform::GooglePlay,
            period_end: info.expiration_time,
        }),
    }
}

pub async fn confirm_android_subscription(
    context: RequestContext<'_, ConfirmAndroidSubscriptionRequest>,
) -> Result<ConfirmAndroidSubscriptionResponse, ServerError<ConfirmAndroidSubscriptionError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    if current_data_cap == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(ConfirmAndroidSubscriptionError::AlreadyPremium));
    }

    let mut billing_lock = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon) => {
            ClientError(ConfirmAndroidSubscriptionError::ConcurrentRequestsAreTooSoon)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    con.set(data_cap(&context.public_key), PREMIUM_TIER_USAGE_SIZE).await?;

    google_play_client::acknowledge_subscription(
        &server_state.android_publisher,
        &server_state.config.google.premium_subscription_product_id,
        &request.purchase_token,
        &context.public_key,
    )
    .await?;

    let purchase = google_play_client::get_subscription(
        &server_state.android_publisher,
        &server_state.config.google.premium_subscription_product_id,
        &request.purchase_token,
    )
        .await
        .map_err(|e| match e {
            SimpleGCPError::Unexpected(msg) => internal!("{:#?}", msg),
        })?;

    billing_lock
        .info
        .push(BillingInfo::GooglePlay(GooglePlayUserInfo {
            purchase_token: request.purchase_token.clone(),
            subscription_product_id: server_state.config.google.premium_subscription_product_id.clone(),
            subscription_offer_id: server_state.config.google.premium_subscription_offer_id.clone(),
            expiration_time : purchase.borrow().expiry_time_millis.as_ref().ok_or_else(|| internal!("Cannot get expiration time of a recovered subscription. public_key {:?}", &context.public_key))?.parse().map_err(|e| internal!("Cannot parse millis into int: {:?}", e))?
        }));
    set_billing_lock(&context.public_key, &mut con, &mut billing_lock).await?;

    Ok(ConfirmAndroidSubscriptionResponse {})
}

pub async fn cancel_subscription(
    context: RequestContext<'_, CancelSubscriptionRequest>,
) -> Result<CancelSubscriptionResponse, ServerError<CancelSubscriptionError>> {
    let server_state = context.server_state;
    let mut con = server_state.index_db_pool.get().await?;

    let fmt_public_key = keys::stringify_public_key(&context.public_key);

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    if current_data_cap == FREE_TIER_USAGE_SIZE {
        return Err(ClientError(CancelSubscriptionError::NotPremium));
    }

    let mut billing_lock = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon) => {
            ClientError(CancelSubscriptionError::ConcurrentRequestsAreTooSoon)
        }
        InternalError(msg) => InternalError(msg),
    })?;

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
            "Cannot downgrade user to free since they are over the data cap. public_key: {}",
            fmt_public_key
        );
        return Err(ClientError(CancelSubscriptionError::UsageIsOverFreeTierDataCap));
    }

    match billing_lock.info.last() {
        None => return Err(internal!("A user somehow has premium tier usage, but no billing information on redis. public_key: {}", fmt_public_key)),
        Some(BillingInfo::GooglePlay(info)) => {
            google_play_client::cancel_subscription(
                &server_state.android_publisher,
                &info.subscription_product_id,
                &info.purchase_token,
            ).await?;
        }
        Some(BillingInfo::Stripe(info)) => {
            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &stripe::SubscriptionId::from_str(&info.subscription_id)?,
            )
                .await
                .map_err(|err| internal!("{:?}", err))?;

            info!("Successfully canceled stripe subscription. public_key: {}", fmt_public_key);
        }
    }

    con.set(data_cap(&context.public_key), FREE_TIER_USAGE_SIZE).await?;
    set_billing_lock(&context.public_key, &mut con, &mut billing_lock).await?;

    Ok(CancelSubscriptionResponse {})
}

pub async fn upgrade_account_stripe(
    context: RequestContext<'_, UpgradeAccountStripeRequest>,
) -> Result<UpgradeAccountStripeResponse, ServerError<UpgradeAccountStripeError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let fmt_public_key = keys::stringify_public_key(&context.public_key);

    info!("Attempting to switch account tier of {} to premium", fmt_public_key);

    let mut con = server_state.index_db_pool.get().await?;

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    if current_data_cap == PREMIUM_TIER_USAGE_SIZE {
        return Err(ClientError(UpgradeAccountStripeError::NewTierIsOldTier));
    }

    let mut billing_lock = lock_billing_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await
    .map_err(|err| match err {
        ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon) => {
            ClientError(UpgradeAccountStripeError::ConcurrentRequestsAreTooSoon)
        }
        InternalError(msg) => InternalError(msg),
    })?;

    let mut maybe_user_info = None;

    for info in billing_lock.info.iter().rev() {
        if let BillingInfo::Stripe(stripe_info) = info {
            maybe_user_info = Some(stripe_info.clone());
            break;
        }
    }

    let (user_info, new_data_cap) = create_subscription(
        server_state,
        &mut con,
        &context.public_key,
        &fmt_public_key,
        &request.account_tier,
        maybe_user_info,
    )
    .await?;

    billing_lock.info.push(BillingInfo::Stripe(user_info));

    con.set(data_cap(&context.public_key), new_data_cap).await?;
    set_billing_lock(&context.public_key, &mut con, &mut billing_lock).await?;

    info!("Successfully switched the account tier of {} from free to premium.", fmt_public_key);

    Ok(UpgradeAccountStripeResponse {})
}

async fn create_subscription(
    server_state: &ServerState, con: &mut deadpool_redis::Connection, public_key: &PublicKey,
    fmt_public_key: &str, account_tier: &StripeAccountTier,
    maybe_user_info: Option<StripeUserInfo>,
) -> Result<(StripeUserInfo, u64), ServerError<UpgradeAccountStripeError>> {
    let (payment_method, data_cap) = match account_tier {
        StripeAccountTier::Premium(payment_method) => (payment_method, PREMIUM_TIER_USAGE_SIZE),
    };

    let (customer_id, customer_name, payment_method_id, last_4) = match payment_method {
        PaymentMethod::NewCard { number, exp_year, exp_month, cvc } => {
            info!("Creating a new card for public_key: {}", fmt_public_key);
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

            info!(
                "Created a new payment method. last_4: {}, public_key: {}",
                last_4, fmt_public_key
            );

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

                    info!("Created customer_id: {}. public_key: {}", customer_id, fmt_public_key);

                    con.json_set(public_key_from_stripe_customer_id(&customer_id), public_key)
                        .await?;

                    (customer_resp.id, customer_name)
                }
                Some(user_info) => {
                    info!(
                        "User already has customer_id: {} public_key: {}",
                        user_info.customer_id, fmt_public_key
                    );

                    let customer_id = stripe::CustomerId::from_str(&user_info.customer_id)?;

                    info!(
                        "Disabling card with a payment method of {} since a new card has just been added. public_key: {}",
                        user_info.customer_id,
                        fmt_public_key
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
                "Creating a setup intent to confirm a users payment method for their subscription. public_key: {}",
                fmt_public_key
            );

            let setup_intent_resp = stripe_client::create_setup_intent(
                &server_state.stripe_client,
                customer_id.clone(),
                payment_method_resp.id.clone(),
            )
            .await?;

            info!(
                "Created a setup intent: {}, public_key: {}",
                setup_intent_resp.id.to_string(),
                fmt_public_key
            );

            (customer_id, customer_name, payment_method_resp.id.to_string(), last_4)
        }
        PaymentMethod::OldCard => {
            info!("Using an old card stored on redis for public_key: {}", fmt_public_key);

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

    info!("Successfully retrieved card for public_key: {}", fmt_public_key);

    let subscription_resp =
        stripe_client::create_subscription(server_state, customer_id.clone(), &payment_method_id)
            .await?;

    info!(
        "Successfully create subscription: {}, public_key: {}",
        subscription_resp.id, fmt_public_key
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

pub async fn get_credit_card(
    context: RequestContext<'_, GetCreditCardRequest>,
) -> Result<GetCreditCardResponse, ServerError<GetCreditCardError>> {
    let mut con = context.server_state.index_db_pool.get().await?;

    info!("Getting credit card for {}", keys::stringify_public_key(&context.public_key));

    let billing_lock: BillingLock = con
        .maybe_json_get(keys::billing_lock(&context.public_key))
        .await?
        .ok_or(ClientError(GetCreditCardError::NoCardAdded))?;

    // Should I get the most recent stripe billing info for this or just check the last card?
    if let Some(BillingInfo::Stripe(info)) = billing_lock.info.last() {
        Ok(GetCreditCardResponse { credit_card_last_4_digits: info.last_4.clone() })
    } else {
        Err(ClientError(GetCreditCardError::NoCardAdded))
    }
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

                let mut billing_hist_size: Option<usize> = None;

                let tx_result = tx!(&mut con, pipe, &[keys::billing_lock(&public_key)], {
                    match lock_billing_workflow(
                        &public_key,
                        &mut con,
                        server_state.config.stripe.millis_between_user_payment_flows,
                    )
                    .await {
                        Ok(billing_lock) => {
                            if let Some(size) = billing_hist_size {
                                if size != billing_lock.info.len() {
                                    return Ok(&mut pipe);
                                }
                            }

                            billing_hist_size = Some(billing_lock.info.len());
                            pipe.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE);
                        }
                        Err(ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon)) => {}
                        Err(err) => {
                            return Err(Abort(internal!("Cannot get billing lock in stripe webhooks: {:#?}", err)));
                        }
                    }
                    Ok(&mut pipe)
                });
                return_if_error!(tx_result);

            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                invoice.billing_reason
            {
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

                let tx_result = tx!(&mut con, pipe, &[keys::billing_lock(&public_key)], {
                    match lock_billing_workflow(
                        &public_key,
                        &mut con,
                        server_state.config.stripe.millis_between_user_payment_flows,
                    )
                    .await {
                        Ok(mut billing_lock) => {
                            if let Some(BillingInfo::Stripe(info)) = billing_lock.info.last_mut() {
                                (*info).expiration_time = subscription_period_end as u64;
                            }

                            billing_lock.last_in_payment_flow = 0;
                            pipe.json_set(keys::billing_lock(&public_key), &billing_lock)
                        }
                        Err(ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon)) => Ok(&mut pipe),
                        Err(err) => Err(Abort(internal!("Cannot get billing lock in stripe webhooks: {:#?}", err)))
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

async fn get_public_key(con: &mut Connection, invoice: &Invoice) -> Result<PublicKey, ServerError<StripeWebhookError>> {
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
            internal!(
                "There is no public_key related to this customer_id: {:?}",
                customer_id
            )
        })?;

    Ok(public_key)
}

#[derive(Debug)]
pub enum GooglePlayWebhookError {
    InvalidToken,
    CannotRetrieveData,
    NoPubSubData,
    CannotDecodePubSubData(DecodeError),
    CannotRetrieveUserInfo,
    CannotRetrievePublicKey,
    CannotParseTime,
}

pub async fn android_notification_webhooks(
    server_state: &Arc<ServerState>, request_body: Bytes, auth_token: String,
) -> Result<(), ServerError<GooglePlayWebhookError>> {
    if !constant_time_eq::constant_time_eq(
        auth_token.as_bytes(),
        server_state.config.google.pubsub_token.as_bytes(),
    ) {
        return Err(ClientError(GooglePlayWebhookError::InvalidToken));
    }

    let message = serde_json::from_slice::<PubsubMessage>(&request_body)?;
    let data = base64::decode(
        message
            .data
            .ok_or(ClientError(GooglePlayWebhookError::NoPubSubData))?,
    )
    .map_err(|e| ClientError(GooglePlayWebhookError::CannotDecodePubSubData(e)))?;

    let notification = serde_json::from_slice::<DeveloperNotification>(&data)?;

    let mut con = server_state.index_db_pool.get().await?;

    if let Some(sub_notif) = notification.subscription_notification {
        let purchase = google_play_client::get_subscription(
            &server_state.android_publisher,
            &sub_notif.subscription_id,
            &sub_notif.purchase_token,
        )
        .await
        .map_err(|e| match e {
            SimpleGCPError::Unexpected(msg) => internal!("{:#?}", msg),
        })?;

        let public_key: PublicKey =
            serde_json::from_str(&purchase.developer_payload.clone().ok_or(internal!(
                "There should be a public key attached to a purchase: {:?}",
                sub_notif.subscription_id
            ))?)?;

        let notification_type = sub_notif.notification_type();
        let mut billing_hist_size: Option<usize> = None;

        let tx_result = tx!(&mut con, pipe, &[keys::billing_lock(&public_key)], {
            match lock_billing_workflow(
                &public_key,
                &mut con,
                server_state.config.stripe.millis_between_user_payment_flows,
            )
            .await {
                Ok(mut billing_lock) => {
                    if let Some(size) = billing_hist_size {
                        if size != billing_lock.info.len() {
                            return Ok(&mut pipe);
                        }
                    }

                    billing_hist_size = Some(billing_lock.info.len());
                    if let Some(BillingInfo::Stripe(info)) = billing_lock.info.last_mut() {
                        match notification_type {
                            NotificationType::SubscriptionRecovered => {
                                billing_lock.last_in_payment_flow = 0;
                                info.expiration_time = purchase.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                pipe.set(data_cap(&public_key), PREMIUM_TIER_USAGE_SIZE).json_set(keys::billing_lock(&public_key), &billing_lock)
                                // give back premium and update time exp
                            }
                            NotificationType::SubscriptionRenewed | NotificationType::SubscriptionRestarted => {
                                billing_lock.last_in_payment_flow = 0;
                                info.expiration_time = purchase.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                pipe.json_set(keys::billing_lock(&public_key), &billing_lock)
                                // change expiry time
                            }
                            NotificationType::SubscriptionInGracePeriod => {
                                Ok(&mut pipe)
                            }
                            NotificationType::SubscriptionExpired
                            | NotificationType::SubscriptionRevoked
                            | NotificationType::SubscriptionOnHold => {
                                billing_lock.last_in_payment_flow = 0;
                                info.expiration_time = purchase.borrow().expiry_time_millis.as_ref().ok_or_else(|| Abort(internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type)))?.parse().map_err(|e| Abort(internal!("Cannot parse millis into int: {:?}", e)))?;
                                pipe.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE).json_set(keys::billing_lock(&public_key), &billing_lock)
                                // remove premium data cap and update exp date to past (for hold)
                            }
                            NotificationType::SubscriptionCanceled
                            | NotificationType::SubscriptionPurchased => Ok(&mut pipe),
                            NotificationType::SubscriptionPriceChangeConfirmed
                            | NotificationType::SubscriptionDeferred
                            | NotificationType::SubscriptionPaused
                            | NotificationType::SubscriptionPausedScheduleChanged => {
                                info!("Unexpected subscription notification: {:?}, public_key: {:?}",
                                    notification_type,
                                    public_key
                                );
                                Ok(&mut pipe)
                            }
                            NotificationType::Unknown => {
                                Err(Abort(internal!(
                                    "Unknown subscription change. public_key: {:?}",
                                    public_key
                                )))
                            }
                        }
                    } else {
                        Err(Abort(internal!("Cannot get any billing info for user. public_key: {:?}", public_key)))
                    }
                }
                Err(ClientError(LockBillingWorkflowError::ConcurrentRequestsAreTooSoon)) => Ok(&mut pipe),
                Err(err) => Err(Abort(internal!("Cannot get billing lock in stripe webhooks: {:#?}", err)))
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
