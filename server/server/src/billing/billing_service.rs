use crate::account_service::GetFileUsageError;
use crate::billing::stripe_client;
use crate::billing::stripe_model::{
    StripeBillingReason, StripeEventType, StripeMaybeContainer, StripeObjectType,
    StripePaymentInfo, StripeSubscriptionInfo, StripeUserInfo, StripeWebhookResponse,
    UNSET_CUSTOMER_ID,
};
use crate::keys::{
    data_cap, public_key_from_stripe_customer_id, stripe_in_billing_workflow, stripe_user_info,
};
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, RequestContext, ServerError, ServerState, StripeClientError,
    FREE_TIER_USAGE_SIZE, MONTHLY_TIER_USAGE_SIZE,
};
use deadpool_redis::redis::AsyncCommands;
use hmac::{Hmac, Mac};
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, GetCreditCardError, GetCreditCardRequest, GetCreditCardResponse, PaymentMethod,
    SwitchAccountTierError, SwitchAccountTierRequest, SwitchAccountTierResponse,
};
use redis_utils::converters::{JsonGet, JsonSet};
use redis_utils::tx;
use sha2::Sha256;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;
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
        .unwrap_or(StripeUserInfo::default());

    let cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let new_data_cap = match (cap, &request.account_tier) {
        (FREE_TIER_USAGE_SIZE, AccountTier::Monthly(card)) => {
            create_subscription(
                server_state,
                &mut con,
                &context.public_key,
                card,
                &mut user_info,
            )
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
                return Err(ClientError(
                    SwitchAccountTierError::CurrentUsageIsMoreThanNewTier,
                ));
            }

            let (active_subscription, active_pos) =
                get_active_subscription::<SwitchAccountTierError>(
                    user_info.subscriptions.as_slice(),
                )?;

            stripe_client::delete_subscription(&server_state, &active_subscription.id).await?;

            user_info.subscriptions[active_pos].is_active = false;

            FREE_TIER_USAGE_SIZE
        }
    };

    let watched_keys = &[
        stripe_user_info(&context.public_key),
        data_cap(&context.public_key),
    ];

    let tx_result = tx!(&mut con, pipe_name, watched_keys, {
        pipe_name
            .set(data_cap(&context.public_key), new_data_cap)
            .del(stripe_in_billing_workflow(&context.public_key))
            .json_set(stripe_user_info(&context.public_key), &user_info)
    });
    return_if_error!(tx_result);

    Ok(SwitchAccountTierResponse {})
}

fn get_active_subscription<E: std::fmt::Debug>(
    subscriptions: &[StripeSubscriptionInfo],
) -> Result<(StripeSubscriptionInfo, usize), ServerError<E>> {
    let active_subscriptions: Vec<&StripeSubscriptionInfo> =
        subscriptions.iter().filter(|info| info.is_active).collect();

    if active_subscriptions.len() > 1 {
        return Err(internal!(
            "Redis says more than one stripe subscription is active: {:?}",
            subscriptions
        ));
    }

    let active_subscription = active_subscriptions
        .get(0)
        .ok_or(internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", subscriptions))?
        .deref()
        .deref()
        .clone();

    let active_pos = subscriptions
        .iter()
        .position(|info| info.is_active)
        .ok_or(internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", subscriptions))?;

    Ok((active_subscription, active_pos))
}

async fn lock_payment_workflow(
    public_key: &PublicKey,
    con: &mut deadpool_redis::Connection,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let tx_result = tx!(con, pipe_name, &[stripe_in_billing_workflow(public_key)], {
        if con.exists(stripe_in_billing_workflow(public_key)).await? {
            return Err(Abort(ClientError(
                SwitchAccountTierError::CurrentlyInBillingWorkflow,
            )));
        }

        pipe_name
            .set(stripe_in_billing_workflow(public_key), 1)
            .expire(stripe_in_billing_workflow(public_key), 5);

        Ok(&mut pipe_name)
    });
    return_if_error!(tx_result);

    Ok(())
}

async fn create_subscription(
    server_state: &ServerState,
    con: &mut deadpool_redis::Connection,
    public_key: &PublicKey,
    payment_method: &PaymentMethod,
    user_info: &mut StripeUserInfo,
) -> Result<u64, ServerError<SwitchAccountTierError>> {
    let chosen_card = match payment_method {
        PaymentMethod::NewCard {
            number,
            exp_year,
            exp_month,
            cvc,
        } => {
            let payment_method_resp = stripe_client::create_payment_method(
                &server_state,
                number,
                exp_year,
                exp_month,
                cvc,
            )
            .await?;

            if user_info.customer_id == UNSET_CUSTOMER_ID {
                user_info.customer_id =
                    stripe_client::create_customer(&server_state, &payment_method_resp.id)
                        .await?
                        .id;

                let tx_result = tx!(
                    con,
                    pipe_name,
                    &[public_key_from_stripe_customer_id(&user_info.customer_id)],
                    {
                        pipe_name.json_set(
                            public_key_from_stripe_customer_id(&user_info.customer_id),
                            public_key,
                        )
                    }
                );
                return_if_error!(tx_result);
            }

            if let Some(info) = user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)
            {
                stripe_client::detach_payment_method_from_customer(&server_state, &info.id).await?;
            }

            stripe_client::create_setup_intent(
                &server_state,
                &user_info.customer_id,
                &payment_method_resp.id,
            )
            .await?;

            user_info.payment_methods.push(StripePaymentInfo {
                id: payment_method_resp.id.clone(),
                last_4: payment_method_resp.card.last4,
                created_at: payment_method_resp.created,
            });

            payment_method_resp.id
        }
        PaymentMethod::OldCard => {
            let old_card = if let Some(info) = user_info
                .payment_methods
                .iter()
                .max_by_key(|info| info.created_at)
            {
                info.id.clone()
            } else {
                return Err(ClientError(SwitchAccountTierError::OldCardDoesNotExist));
            };

            old_card
        }
    };

    let subscription_resp = match stripe_client::create_subscription(
        &server_state,
        &user_info.customer_id,
        &chosen_card,
    )
    .await
    {
        Ok(resp) => resp,
        Err(StripeClientError::Other(e)) => return Err(InternalError(e)),
        Err(e) => {
            match payment_method {
                PaymentMethod::NewCard { .. } => {
                    stripe_client::delete_customer(&server_state, &user_info.customer_id).await?;
                }
                PaymentMethod::OldCard => {}
            }
            return Err(ServerError::<SwitchAccountTierError>::from(e));
        }
    };

    user_info.subscriptions.push(StripeSubscriptionInfo {
        id: subscription_resp.id,
        period_end: subscription_resp.current_period_end,
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

    Ok(GetCreditCardResponse {
        credit_card_last_4_digits: payment_method.last_4.clone(),
    })
}

#[derive(Debug)]
pub enum StripeWebhookError {
    VerificationError(String),
    InvalidHeader(String),
    InvalidBody(String),
}

pub async fn stripe_webhooks(
    server_state: &Arc<ServerState>,
    request_body: Bytes,
    stripe_sig: HeaderValue,
) -> Result<(), ServerError<StripeWebhookError>> {
    // use ServerError so you can use internal macro
    let webhook = match serde_json::from_slice::<StripeWebhookResponse>(request_body.as_ref()) {
        Ok(webhook) => webhook,
        Err(e) => {
            return Err(ClientError(StripeWebhookError::InvalidBody(format!(
                "Cannot deserialize stripe webhook body: {:?}",
                e
            ))))
        }
    };

    let event_type = match webhook.event_type {
        StripeMaybeContainer::Expected(ref known) => known,
        StripeMaybeContainer::Unexpected(_) => return Ok(()),
    };

    verify_stripe_webhook(server_state, &request_body, stripe_sig)?;

    let mut con = server_state.index_db_pool.get().await?;

    match (event_type, &webhook.data.object) {
        (StripeEventType::InvoicePaymentFailed, StripeObjectType::Invoice(invoice)) => {
            if let StripeBillingReason::SubscriptionCycle = invoice.billing_reason {
                let public_key: PublicKey = con
                    .maybe_json_get(public_key_from_stripe_customer_id(&invoice.customer_id))
                    .await?
                    .ok_or(internal!(
                        "There is no public key related to this customer id: {}",
                        invoice.customer_id
                    ))?;

                con.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE).await?;
            } else {
                return Ok(());
            }
        }
        (StripeEventType::InvoicePaid, StripeObjectType::Invoice(partial_invoice)) => {
            match partial_invoice.billing_reason {
                StripeBillingReason::SubscriptionCycle => {
                    let invoice =
                        stripe_client::retrieve_invoice(&server_state, &partial_invoice.id)
                            .await
                            .map_err(|e| internal!("Cannot retrieve expanded invoice: {:?}", e))?;

                    let subscription = match invoice.subscription {
                        StripeMaybeContainer::Unexpected(_) => {
                            return Err(internal!(
                                "There should be a subscription tied to this invoice: {:?}",
                                invoice
                            ))
                        }
                        StripeMaybeContainer::Expected(subscription) => subscription,
                    };

                    let public_key: PublicKey = con
                        .maybe_json_get(public_key_from_stripe_customer_id(&invoice.customer_id))
                        .await?
                        .ok_or(internal!(
                            "There is no public key related to this customer id: {}",
                            invoice.customer_id
                        ))?;

                    let mut user_info: StripeUserInfo = con
                        .maybe_json_get(stripe_user_info(&public_key))
                        .await?
                        .ok_or(internal!(
                            "There is no stripe user info related to this public key: {:?}",
                            public_key
                        ))?;

                    let active_pos = user_info
                        .subscriptions
                        .iter()
                        .position(|info| info.is_active)
                        .ok_or(internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", user_info))?;

                    user_info.subscriptions[active_pos].period_end =
                        subscription.current_period_end;
                }
                _ => return Ok(()),
            }
        }
        (_, StripeObjectType::Unmatched(_)) => {
            return Err(internal!("Unmatched webhook: {:?}", webhook))
        }
    };

    Ok(())
}

fn verify_stripe_webhook(
    state: &Arc<ServerState>,
    request_body: &Bytes,
    stripe_sig: HeaderValue,
) -> Result<(), ServerError<StripeWebhookError>> {
    let json = match std::str::from_utf8(&request_body.as_ref()) {
        Ok(json) => json,
        Err(e) => {
            return Err(ClientError(StripeWebhookError::InvalidBody(format!(
                "Cannot turn stripe webhook body bytes into str: {:?}",
                e
            ))))
        }
    };

    let sig_header_split = stripe_sig
        .to_str()
        .map_err(|e| {
            ClientError(StripeWebhookError::InvalidHeader(format!(
                "Cannot turn stripe webhook header into str: {:?}",
                e
            )))
        })?
        .split(",");

    let mut maybe_time = None;
    let mut maybe_expected_sig = None;

    for part in sig_header_split {
        let mut part_split = part.split("=");

        let part_title =
            part_split
                .next()
                .ok_or(ClientError(StripeWebhookError::InvalidHeader(format!(
                    "Cannot get the component from the header to verify stripe webhook: {:?}",
                    stripe_sig
                ))))?;

        let part_value =
            part_split
                .last()
                .ok_or(ClientError(StripeWebhookError::InvalidHeader(format!(
                    "Cannot get the component from the header to verify stripe webhook: {:?}",
                    stripe_sig
                ))))?;

        if part_title == "t" {
            maybe_time = Some(part_value);
        } else if part_title == "v1" {
            maybe_expected_sig = Some(part_value);
        }
    }

    let time = match maybe_time {
        None => {
            return Err(ClientError(StripeWebhookError::InvalidHeader(format!(
                "Cannot get header information for verification: {:?}",
                stripe_sig
            ))))
        }
        Some(time) => time,
    };

    let expected_sig = match maybe_expected_sig {
        None => {
            return Err(ClientError(StripeWebhookError::InvalidHeader(format!(
                "Cannot get header information for verification: {:?}",
                stripe_sig
            ))))
        }
        Some(expected_sig) => expected_sig,
    };

    let signed_payload = format!("{}.{}", time, json);

    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.stripe.signing_secret.as_bytes())
        .map_err(|e| internal!("Cannot create hmac for verifying json: {:?}", e))?;

    mac.update(signed_payload.as_bytes());

    let expected_sig_bytes = hex::decode(expected_sig).map_err(|e| {
        ClientError(StripeWebhookError::VerificationError(format!(
            "Could not verify stripe webhook: {}",
            e
        )))
    })?;

    mac.verify_slice(expected_sig_bytes.as_slice())
        .map_err(|e| {
            ClientError(StripeWebhookError::VerificationError(format!(
                "Could not verify stripe webhook: {}",
                e
            )))
        })
}
