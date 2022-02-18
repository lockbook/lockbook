use crate::account_service::GetFileUsageError;
use crate::billing::stripe_client;
use crate::billing::stripe_model::{StripePaymentInfo, StripeSubscriptionInfo, StripeUserInfo};
use crate::keys::{
    data_cap, public_key_from_stripe_customer_id, stripe_in_billing_workflow, stripe_user_info,
};
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, RequestContext, ServerError, ServerState, SimplifiedStripeError,
    FREE_TIER_USAGE_SIZE, MONTHLY_TIER_USAGE_SIZE,
};
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::Connection;
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, GetCreditCardError, GetCreditCardRequest, GetCreditCardResponse, PaymentMethod,
    SwitchAccountTierError, SwitchAccountTierRequest, SwitchAccountTierResponse,
};
use redis_utils::converters::{JsonGet, JsonSet};
use redis_utils::tx;
use std::fmt::Debug;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use stripe::{Expandable, Object, WebhookEvent};
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

pub async fn switch_account_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>,
) -> Result<SwitchAccountTierResponse, ServerError<SwitchAccountTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut con = server_state.index_db_pool.get().await?;

    lock_payment_workflow(&context.public_key, &mut con).await?;

    let mut user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&context.public_key))
        .await?
        .unwrap_or_default();

    let current_data_cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let new_data_cap = match (current_data_cap, &request.account_tier) {
        (FREE_TIER_USAGE_SIZE, AccountTier::Monthly(card)) => {
            create_subscription(server_state, &mut con, &context.public_key, card, &mut user_info)
                .await?
        }
        (FREE_TIER_USAGE_SIZE, AccountTier::Free) | (_, AccountTier::Monthly(_)) => {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }
        (_, AccountTier::Free) => {
            let usage: u64 = account_service::get_file_usage(&mut con, &context.public_key)
                .await
                .map_err(|e| match e {
                    GetFileUsageError::UserNotFound => {
                        ClientError(SwitchAccountTierError::UserNotFound)
                    }
                    GetFileUsageError::Internal(e) => ServerError::from(e),
                })?
                .iter()
                .map(|a| a.size_bytes)
                .sum();

            if usage > FREE_TIER_USAGE_SIZE {
                return Err(ClientError(SwitchAccountTierError::CurrentUsageIsMoreThanNewTier));
            }

            let pos = get_active_subscription_index(&user_info.subscriptions)?;

            stripe_client::cancel_subscription(
                &server_state.stripe_client,
                &stripe::SubscriptionId::from_str(&user_info.subscriptions[pos].id)?,
            )
            .await?;

            user_info.subscriptions[pos].is_active = false;

            FREE_TIER_USAGE_SIZE
        }
    };

    con.del(stripe_in_billing_workflow(&context.public_key))
        .await?;
    con.set(data_cap(&context.public_key), new_data_cap).await?;
    con.json_set(stripe_user_info(&context.public_key), &user_info)
        .await?;

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
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let tx_result = tx!(con, pipe_name, &[stripe_in_billing_workflow(public_key)], {
        if con.exists(stripe_in_billing_workflow(public_key)).await? {
            return Err(Abort(ClientError(SwitchAccountTierError::CurrentlyInBillingWorkflow)));
        }

        pipe_name
            .set(stripe_in_billing_workflow(public_key), 1)
            .expire(stripe_in_billing_workflow(public_key), 30);

        Ok(&mut pipe_name)
    });
    return_if_error!(tx_result);

    Ok(())
}

async fn create_subscription(
    server_state: &ServerState, con: &mut deadpool_redis::Connection, public_key: &PublicKey,
    payment_method: &PaymentMethod, user_info: &mut StripeUserInfo,
) -> Result<u64, ServerError<SwitchAccountTierError>> {
    let (customer_id, payment_method_id) = match payment_method {
        PaymentMethod::NewCard { number, exp_year, exp_month, cvc } => {
            let payment_method_resp = stripe_client::create_payment_method(
                &server_state.stripe_client,
                number,
                *exp_month,
                *exp_year,
                cvc,
            )
            .await?;

            let customer_id = match &user_info.customer_id {
                None => {
                    let customer_resp = stripe_client::create_customer(
                        &server_state.stripe_client,
                        payment_method_resp.id(),
                    )
                    .await?;
                    let customer_id = customer_resp.id.to_string();

                    con.json_set(public_key_from_stripe_customer_id(&customer_id), public_key)
                        .await?;

                    user_info.customer_id = Some(customer_id);
                    customer_resp.id
                }
                Some(customer_id) => stripe::CustomerId::from_str(customer_id)?,
            };

            if let Some(info) = user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)
            {
                stripe_client::detach_payment_method_from_customer(
                    &server_state.stripe_client,
                    &stripe::PaymentMethodId::from_str(&info.id)?,
                )
                .await?;
            }

            stripe_client::create_setup_intent(
                &server_state.stripe_client,
                customer_id.clone(),
                payment_method_resp.id(),
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

            user_info.payment_methods.push(StripePaymentInfo {
                id: customer_id.to_string(),
                last_4,
                created_at: payment_method_resp.created as u64,
            });

            (customer_id, payment_method_resp.id.to_string())
        }
        PaymentMethod::OldCard => {
            match (&user_info.customer_id, user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)) {
                (Some(customer_id), Some(payment_info)) => (stripe::CustomerId::from_str(customer_id)?, payment_info.id.clone()),
                (Some(_), None) | (None, None) => return Err(ClientError(SwitchAccountTierError::OldCardDoesNotExist)),
                (None, Some(_)) => return Err(internal!("User info is in a mismatched state where payment information exists despite no customer existing: {:?}", user_info)),
            }
        }
    };

    let subscription_resp = match stripe_client::create_subscription(
        server_state,
        customer_id.clone(),
        &payment_method_id,
    )
    .await
    {
        Ok(resp) => resp,
        Err(SimplifiedStripeError::Other(e)) => return Err(InternalError(e)),
        Err(e) => {
            match payment_method {
                PaymentMethod::NewCard { .. } if user_info.payment_methods.len() == 1 => {
                    stripe_client::delete_customer(&server_state.stripe_client, &customer_id)
                        .await?;
                }
                _ => {}
            }

            return Err(ServerError::<SwitchAccountTierError>::from(e));
        }
    };

    user_info.subscriptions.push(StripeSubscriptionInfo {
        id: subscription_resp.id.to_string(),
        period_end: subscription_resp.current_period_end as u64,
        is_active: true,
    });

    Ok(MONTHLY_TIER_USAGE_SIZE)
}

pub async fn get_credit_card(
    context: RequestContext<'_, GetCreditCardRequest>,
) -> Result<GetCreditCardResponse, ServerError<GetCreditCardError>> {
    let mut con = context.server_state.index_db_pool.get().await?;

    let user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&context.public_key))
        .await?
        .ok_or(ClientError(GetCreditCardError::NotAStripeCustomer))?;

    let payment_method = user_info
        .payment_methods
        .iter()
        .max_by_key(|info| info.created_at)
        .ok_or(internal!(
            "No payment method on stripe user info, although there should be at least 1: {:?}",
            user_info
        ))?;

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
    let event =
        stripe::Webhook::construct_event(payload, sig, &server_state.config.stripe.signing_secret)?;

    let mut con = server_state.index_db_pool.get().await?;

    match (&event.event_type, &event.data.object) {
        (stripe::EventType::InvoicePaymentFailed, stripe::EventObject::Invoice(invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                invoice.billing_reason.as_deref()
            {
                let customer_id = match invoice
                    .customer
                    .as_ref()
                    .ok_or_else(|| {
                        ClientError(StripeWebhookError::InvalidBody(
                            "Cannot retrieve the customer id.".to_string(),
                        ))
                    })?
                    .deref()
                {
                    Expandable::Id(id) => id.to_string(),
                    Expandable::Object(customer) => customer.id.to_string(),
                };

                let (public_key, user_info) =
                    get_public_key_and_stripe_user_info(&event, &mut con, &customer_id).await?;

                con.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE).await?;
                con.json_set(stripe_user_info(&public_key), &user_info)
                    .await?;
            }
        }
        (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(partial_invoice)) => {
            if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                partial_invoice.billing_reason.as_deref()
            {
                let invoice = stripe_client::retrieve_invoice(
                    &server_state.stripe_client,
                    &partial_invoice.id,
                )
                .await
                .map_err(|e| {
                    internal!(
                        "While trying to get the expanded invoice, an error was encountered: {:?}",
                        e
                    )
                })?;

                let subscription_period_end = match invoice.subscription.as_deref() {
                    None => {
                        return Err(internal!(
                            "There should be a subscription tied to this invoice: {:?}",
                            invoice
                        ))
                    }
                    Some(Expandable::Id(_)) => {
                        return Err(internal!(
                            "The subscription should be expanded in this invoice: {:?}",
                            invoice
                        ))
                    }
                    Some(Expandable::Object(subscription)) => subscription.current_period_end,
                };

                let customer_id = match invoice
                    .customer
                    .ok_or_else(|| {
                        ClientError(StripeWebhookError::InvalidBody(
                            "Cannot retrieve the customer id.".to_string(),
                        ))
                    })?
                    .deref()
                {
                    Expandable::Id(id) => id.to_string(),
                    Expandable::Object(customer) => customer.id.to_string(),
                };

                let (public_key, mut user_info) =
                    get_public_key_and_stripe_user_info(&event, &mut con, &customer_id).await?;
                let pos = get_active_subscription_index(&user_info.subscriptions)?;

                user_info.subscriptions[pos].period_end = subscription_period_end as u64;

                con.json_set(stripe_user_info(&public_key), &user_info)
                    .await?;
            }
        }
        (_, _) => println!("Unmatched endpoint: {:?}", event.event_type),
    }

    Ok(())
}

async fn get_public_key_and_stripe_user_info(
    event: &WebhookEvent, con: &mut Connection, customer_id: &str,
) -> Result<(PublicKey, StripeUserInfo), ServerError<StripeWebhookError>> {
    let public_key: PublicKey = con
        .maybe_json_get(public_key_from_stripe_customer_id(customer_id))
        .await?
        .ok_or(internal!(
            "There is no public key related to this customer id: {:?}",
            customer_id
        ))?;

    let user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&public_key))
        .await?
        .ok_or(internal!(
            "Payment failed for a customer we don't have info about on redis: {:?}",
            event
        ))?;

    Ok((public_key, user_info))
}
