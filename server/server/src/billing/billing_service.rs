use crate::account_service::GetUsageHelperError;
use crate::billing::stripe_client;
use crate::billing::stripe_model::{
    StripePaymentInfo, StripeSubscriptionInfo, StripeUserInfo, Timestamp,
};
use crate::keys::{data_cap, public_key_from_stripe_customer_id, stripe_user_info};
use crate::ServerError::ClientError;
use crate::{
    account_service, keys, RequestContext, ServerError, ServerState, FREE_TIER_USAGE_SIZE,
    PREMIUM_TIER_USAGE_SIZE,
};
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::Connection;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::{
    AccountTier, GetCreditCardError, GetCreditCardRequest, GetCreditCardResponse, PaymentMethod,
    SwitchAccountTierError, SwitchAccountTierRequest, SwitchAccountTierResponse,
};
use log::info;
use redis_utils::converters::{JsonGet, JsonSet, PipelineJsonSet};
use redis_utils::tx;
use std::fmt::Debug;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

pub async fn switch_account_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>,
) -> Result<SwitchAccountTierResponse, ServerError<SwitchAccountTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let fmt_public_key = keys::stringify_public_key(&context.public_key);
    let fmt_new_tier =
        if let AccountTier::Premium(_) = request.account_tier { "premium" } else { "free" };

    info!("Attempting to switch account tier of {} to {}", fmt_public_key, fmt_new_tier);

    let mut con = server_state.index_db_pool.get().await?;

    let mut user_info = lock_payment_workflow(
        &context.public_key,
        &mut con,
        server_state.config.stripe.millis_between_user_payment_flows,
    )
    .await?;

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let new_data_cap = match (current_data_cap, &request.account_tier) {
        (FREE_TIER_USAGE_SIZE, AccountTier::Premium(card)) => {
            create_subscription(
                server_state,
                &mut con,
                &context.public_key,
                &fmt_public_key,
                card,
                &mut user_info,
            )
            .await?
        }
        (FREE_TIER_USAGE_SIZE, AccountTier::Free)
        | (PREMIUM_TIER_USAGE_SIZE, AccountTier::Premium(_)) => {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }
        (PREMIUM_TIER_USAGE_SIZE, AccountTier::Free) => {
            info!("Switching account tier to free. public_key: {}", fmt_public_key);

            let usage: u64 = account_service::get_usage_helper(&mut con, &context.public_key)
                .await
                .map_err(|e| match e {
                    GetUsageHelperError::UserNotFound => {
                        ClientError(SwitchAccountTierError::UserNotFound)
                    }
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
                return Err(ClientError(SwitchAccountTierError::CurrentUsageIsMoreThanNewTier));
            }

            let pos = get_active_subscription_index(&user_info.subscriptions)?;

            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &stripe::SubscriptionId::from_str(&user_info.subscriptions[pos].id)?,
            )
            .await?;

            info!("Successfully canceled stripe subscription. public_key: {}", fmt_public_key);

            user_info.subscriptions[pos].is_active = false;

            FREE_TIER_USAGE_SIZE
        }
        (_, AccountTier::Free) | (_, AccountTier::Premium(_)) => {
            return Err(internal!(
                "Unrecognized current data cap: {}, public_key: {}",
                current_data_cap,
                fmt_public_key
            ));
        }
    };

    user_info.last_in_payment_flow = 0;

    con.set(data_cap(&context.public_key), new_data_cap).await?;
    con.json_set(stripe_user_info(&context.public_key), &user_info)
        .await?;

    info!(
        "Successfully switched the account tier of {} from {} to {}.",
        fmt_public_key,
        if current_data_cap == PREMIUM_TIER_USAGE_SIZE {
            "premium"
        } else if current_data_cap == FREE_TIER_USAGE_SIZE {
            "free"
        } else {
            "unknown"
        },
        fmt_new_tier
    );

    Ok(SwitchAccountTierResponse {})
}

fn get_active_subscription_index<U: Debug>(
    subscriptions: &[StripeSubscriptionInfo],
) -> Result<usize, ServerError<U>> {
    let active_pos = subscriptions
        .iter()
        .position(|info| info.is_active)
        .ok_or_else(|| internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", subscriptions))?;

    Ok(active_pos)
}

async fn lock_payment_workflow(
    public_key: &PublicKey, con: &mut deadpool_redis::Connection,
    millis_between_payment_flows: Timestamp,
) -> Result<StripeUserInfo, ServerError<SwitchAccountTierError>> {
    let mut user_info = StripeUserInfo::default();

    let tx_result = tx!(con, pipe, &[stripe_user_info(public_key)], {
        user_info = con
            .maybe_json_get(stripe_user_info(public_key))
            .await?
            .unwrap_or_default();

        let current_time = get_time().0 as Timestamp;

        if current_time - user_info.last_in_payment_flow < millis_between_payment_flows {
            info!(
                "User is already in payment flow, or this request is too soon after a failed one. public_key: {}",
                keys::stringify_public_key(public_key)
            );
            return Err(Abort(ClientError(SwitchAccountTierError::ConcurrentRequestsAreTooSoon)));
        }

        user_info.last_in_payment_flow = current_time;

        pipe.json_set(stripe_user_info(public_key), &user_info)?;

        Ok(&mut pipe)
    });
    return_if_error!(tx_result);

    info!(
        "User successfully entered payment flow. public_key: {}",
        keys::stringify_public_key(public_key)
    );

    Ok(user_info)
}

async fn create_subscription(
    server_state: &ServerState, con: &mut deadpool_redis::Connection, public_key: &PublicKey,
    fmt_public_key: &str, payment_method: &PaymentMethod, user_info: &mut StripeUserInfo,
) -> Result<u64, ServerError<SwitchAccountTierError>> {
    let (customer_id, payment_method_id) = match payment_method {
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

            let customer_id = match &user_info.customer_id {
                None => {
                    info!(
                        "User has no customer_id. Creating one with stripe now. public_key: {}",
                        keys::stringify_public_key(public_key)
                    );

                    let customer_resp = stripe_client::create_customer(
                        &server_state.stripe_client,
                        &user_info.customer_name.to_string(),
                        payment_method_resp.id.clone(),
                    )
                    .await?;
                    let customer_id = customer_resp.id.to_string();

                    info!("Created customer_id: {}. public_key: {}", customer_id, fmt_public_key);

                    con.json_set(public_key_from_stripe_customer_id(&customer_id), public_key)
                        .await?;

                    user_info.customer_id = Some(customer_id);
                    customer_resp.id
                }
                Some(customer_id) => {
                    info!(
                        "User already has customer_id: {} public_key: {}",
                        customer_id, fmt_public_key
                    );

                    stripe::CustomerId::from_str(customer_id)?
                }
            };

            if let Some(info) = user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)
            {
                info!(
                    "Disabling card with a payment method of {} since a new card has just been added. public_key: {}",
                    info.id,
                    fmt_public_key
                );

                stripe_client::detach_payment_method_from_customer(
                    &server_state.stripe_client,
                    &stripe::PaymentMethodId::from_str(&info.id)?,
                )
                .await?;
            }

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

            user_info.payment_methods.push(StripePaymentInfo {
                id: customer_id.to_string(),
                last_4,
                created_at: payment_method_resp.created as u64,
            });

            (customer_id, payment_method_resp.id.to_string())
        }
        PaymentMethod::OldCard => {
            info!("Using an old card stored on redis for public_key: {}", fmt_public_key);

            let payment_method = user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)
                .ok_or(ClientError(SwitchAccountTierError::OldCardDoesNotExist))?;

            match &user_info.customer_id {
                Some(customer_id) => (stripe::CustomerId::from_str(customer_id)?, payment_method.id.clone()),
                None => return Err(internal!("StripeUserInfo is in an inconsistent state: has payment method but no customer id: {:?}", user_info))
            }
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

    user_info.subscriptions.push(StripeSubscriptionInfo {
        id: subscription_resp.id.to_string(),
        period_end: subscription_resp.current_period_end as u64,
        is_active: true,
    });

    Ok(PREMIUM_TIER_USAGE_SIZE)
}

pub async fn get_credit_card(
    context: RequestContext<'_, GetCreditCardRequest>,
) -> Result<GetCreditCardResponse, ServerError<GetCreditCardError>> {
    let mut con = context.server_state.index_db_pool.get().await?;

    info!("Getting credit card for {}", keys::stringify_public_key(&context.public_key));

    let user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&context.public_key))
        .await?
        .ok_or(ClientError(GetCreditCardError::NotAStripeCustomer))?;

    let payment_method = user_info
        .payment_methods
        .iter()
        .max_by_key(|info| info.created_at)
        .ok_or(ClientError(GetCreditCardError::NotAStripeCustomer))?;

    Ok(GetCreditCardResponse { credit_card_last_4_digits: payment_method.last_4.clone() })
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

                let (public_key, user_info) =
                    get_public_key_and_stripe_user_info(&event, &mut con, &customer_id).await?;

                info!(
                    "User tier being reduced due to failed renewal payment via stripe. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );

                con.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE).await?;
                con.json_set(stripe_user_info(&public_key), &user_info)
                    .await?;
            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(partial_invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                partial_invoice.billing_reason
            {
                let invoice = stripe_client::retrieve_invoice(
                    &server_state.stripe_client,
                    &partial_invoice.id,
                )
                .await
                .map_err(|e| internal!("Error expanding invoice: {:?}", e))?;

                let subscription_period_end = match invoice.subscription {
                    None => {
                        return Err(internal!(
                            "There should be a subscription tied to this invoice: {:?}",
                            invoice
                        ))
                    }
                    Some(stripe::Expandable::Id(_)) => {
                        return Err(internal!(
                            "The subscription should be expanded in this invoice: {:?}",
                            invoice
                        ))
                    }
                    Some(stripe::Expandable::Object(subscription)) => {
                        subscription.current_period_end
                    }
                };

                let customer_id = match invoice.customer.ok_or_else(|| {
                    ClientError(StripeWebhookError::InvalidBody(
                        "Cannot retrieve the customer_id.".to_string(),
                    ))
                })? {
                    stripe::Expandable::Id(id) => id.to_string(),
                    stripe::Expandable::Object(customer) => customer.id.to_string(),
                };

                let (public_key, mut user_info) =
                    get_public_key_and_stripe_user_info(&event, &mut con, &customer_id).await?;
                let pos = get_active_subscription_index(&user_info.subscriptions)?;

                info!(
                    "User's subscription period_end is being changed after successful renewal. public_key: {}",
                    keys::stringify_public_key(&public_key)
                );

                user_info.subscriptions[pos].period_end = subscription_period_end as u64;

                con.json_set(stripe_user_info(&public_key), &user_info)
                    .await?;
            }
        }
        (_, _) => {
            return Err(internal!("Unexpected and unhandled stripe event: {:?}", event.event_type))
        }
    }

    Ok(())
}

async fn get_public_key_and_stripe_user_info(
    event: &stripe::WebhookEvent, con: &mut Connection, customer_id: &str,
) -> Result<(PublicKey, StripeUserInfo), ServerError<StripeWebhookError>> {
    let public_key: PublicKey = con
        .maybe_json_get(public_key_from_stripe_customer_id(customer_id))
        .await?
        .ok_or_else(|| {
            internal!("There is no public_key related to this customer_id: {:?}", customer_id)
        })?;

    let user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&public_key))
        .await?
        .ok_or_else(|| {
            internal!(
                "Payment failed for a customer we don't have info about on redis: {:?}",
                event
            )
        })?;

    Ok((public_key, user_info))
}
