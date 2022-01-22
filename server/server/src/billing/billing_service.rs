use crate::account_service::GetFileUsageError;
use crate::billing::billing_service::StripeWebhookError::{
    InvalidBody, InvalidHeader, VerificationError,
};
use crate::billing::stripe_client;
use crate::billing::stripe_model::{
    StripeBillingReason, StripeEventType, StripeMaybeContainer, StripeObjectType,
    StripeSubscriptionInfo, StripeUserInfo, StripeWebhookResponse, UNSET_CUSTOMER_ID,
};
use crate::keys::{
    data_cap, owned_files, public_key, public_key_from_stripe_customer_id,
    stripe_in_billing_workflow, stripe_user_info,
};
use crate::ServerError::{ClientError, InternalError};
use crate::{
    account_service, keys, RequestContext, ServerError, ServerState, StripeClientError,
    FREE_TIER_USAGE_SIZE, MONTHLY_TIER_USAGE_SIZE,
};
use deadpool_redis::Connection;
use hmac::{Hmac, Mac};
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, FileUsage, GetCreditCardError, GetCreditCardRequest, GetCreditCardResponse,
    PaymentMethod, SwitchAccountTierError, SwitchAccountTierRequest, SwitchAccountTierResponse,
};
use lockbook_models::tree::FileMetaExt;
use redis::AsyncCommands;
use redis_utils::converters::JsonGet;
use redis_utils::tx;
use sha2::Sha256;
use std::ops::Index;
use std::sync::Arc;
use uuid::Uuid;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

pub async fn switch_account_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>,
) -> Result<SwitchAccountTierResponse, ServerError<SwitchAccountTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut con = server_state.index_db_pool.get().await?;

    lock_payment_workflow::<SwitchAccountTierError>(&context.public_key, &mut con);

    let mut user_info: StripeUserInfo = con
        .maybe_json_get(stripe_user_info(&context.public_key))
        .await?
        .unwrap_or(StripeUserInfo::default());

    let cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let new_data_cap = match (cap, &request.account_tier) {
        (FREE_TIER_USAGE_SIZE, AccountTier::Monthly(card)) => {
            create_subscription(server_state, &context.public_key, card, &mut user_info).await?
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

            let (active_pos, active_subscription) =
                get_active_subscription::<SwitchAccountTierError>(
                    user_info.subscriptions.as_slice(),
                )?;

            stripe_client::delete_subscription(&server_state, &active_subscription.id).await?;

            user_info.subscriptions[active_pos].is_active = false;

            FREE_TIER_USAGE_SIZE
        }
    };

    let watched_keys = &[
        data_cap(&context.public_key),
        stripe_user_info(&context.public_key),
    ];

    let tx_result = tx!(&mut con, pipe_name, watched_keys, {
        pipe_name
            .set(data_cap(&request.public_key), new_data_cap)
            .json_set(stripe_user_info(public_key), &user_info)
    });
    return_if_error!(tx_result);

    Ok(SwitchAccountTierResponse {})
}

fn get_active_subscription<E>(
    subscriptions: &[StripeSubscriptionInfo],
) -> Result<(StripeSubscriptionInfo, usize), ServerError<E>> {
    let active_subscriptions: Vec<StripeSubscriptionInfo> =
        subscriptions.iter().filter(|info| info.is_active).collect();

    if active_subscriptions.len() > 1 {
        return Err(internal!(
            "Redis says more than one stripe subscription is active: {:?}",
            user_info
        ));
    }

    let active_subscription = active_subscriptions
        .get(0)
        .ok_or(internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", user_info))?
        .clone();

    if active_index != user_info.subscriptions.len() - 1 {
        return Err(internal!(
            "Latest subscription is not the active one: {:?}",
            user_info
        ));
    }

    let active_pos = subscriptions
        .iter()
        .position(|info| info.is_active)
        .ok_or(internal!("Redis says there is no active subscription despite the user having non free data cap: {:?}", user_info))?;

    Ok((active_subscription, active_pos))
}

fn lock_payment_workflow<E>(
    public_key: &PublicKey,
    con: &mut Connection,
) -> Result<(), ServerError<E>> {
    let tx_result = tx!(
        &mut con,
        pipe_name,
        &[stripe_in_billing_workflow(&context.public_key)],
        {
            let key = stripe_in_billing_workflow(public_key);

            if con.exists(key).await? {
                return Err(Abort(ClientError(
                    SwitchAccountTierError::CurrentlyInBillingWorkflow,
                )));
            }

            pipe_name.set(key, 1).expire(key, 5)
        }
    );
    return_if_error!(tx_result);
}

async fn create_subscription(
    server_state: &ServerState,
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
                    &mut con,
                    pipe_name,
                    &[public_key_from_stripe_customer_id(&user_info.customer_id)],
                    {
                        pipe_name.json_set(
                            public_key_from_stripe_customer_id(&user_info.customer_id),
                            public_key,
                        )?
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
                &customer_id,
                &payment_method_resp.id,
            )
            .await?;

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

    let subscription_resp =
        match stripe_client::create_subscription(&server_state, &chosen_card, &payment_method_id)
            .await
        {
            Ok(resp) => resp,
            Err(StripeClientError::Other(e)) => return Err(InternalError(e)),
            Err(e) => {
                match payment_method {
                    PaymentMethod::NewCard { .. } => {
                        stripe_client::delete_customer(&server_state, &customer_id).await?;
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
        .into_iter()
        .max_by_key(|info| info.created_at)
        .ok_or(GetCreditCardError::OldCardDoesNotExist)?;

    Ok(GetCreditCardResponse {
        credit_card_last_4_digits: payment_method.last_4,
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
        Err(e) => return Err(internal!("Cannot deserialize stripe webhook body: {:?}", e)),
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

                con.set(data_cap(&public_key), FREE_TIER_USAGE_SIZE);
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
                        .ok_or(ClientError(GetCreditCardError::NotAStripeCustomer))?;

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
) -> Result<(), StripeWebhookError> {
    let json = match std::str::from_utf8(&request_body.as_ref()) {
        Ok(json) => json,
        Err(e) => {
            return Err(InvalidBody(format!(
                "Cannot turn stripe webhook body bytes into str: {:?}",
                e
            )))
        }
    };

    let sig_header_split = stripe_sig
        .to_str()
        .map_err(|e| {
            InvalidHeader(format!(
                "Cannot turn stripe webhook header into str: {:?}",
                e
            ))
        })?
        .split(",");

    let mut maybe_time = None;
    let mut maybe_expected_sig = None;

    for part in sig_header_split {
        let mut part_split = part.split("=");

        let part_title = part_split.next().ok_or(InvalidHeader(format!(
            "Cannot get the component from the header to verify stripe webhook: {:?}",
            stripe_sig
        )))?;

        let part_value = part_split.last().ok_or(InvalidHeader(format!(
            "Cannot get the component from the header to verify stripe webhook: {:?}",
            stripe_sig
        )))?;

        if part_title == "t" {
            maybe_time = Some(part_value);
        } else if part_title == "v1" {
            maybe_expected_sig = Some(part_value);
        }
    }

    let time = match maybe_time {
        None => {
            return Err(InvalidHeader(format!(
                "Cannot get header information for verification: {:?}",
                stripe_sig
            )))
        }
        Some(time) => time,
    };

    let expected_sig = match maybe_expected_sig {
        None => {
            return Err(InvalidHeader(format!(
                "Cannot get header information for verification: {:?}",
                stripe_sig
            )))
        }
        Some(expected_sig) => expected_sig,
    };

    let signed_payload = format!("{}.{}", time, json);

    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.stripe.signing_secret.as_bytes())
        .map_err(|e| {
            StripeWebhookError::InternalError(format!(
                "Cannot create hmac for verifying json: {:?}",
                e
            ))
        })?;

    mac.update(signed_payload.as_bytes());

    let expected_sig_bytes = hex::decode(expected_sig)
        .map_err(|e| VerificationError(format!("Could not verify stripe webhook: {}", e)))?;

    mac.verify_slice(expected_sig_bytes.as_slice())
        .map_err(|e| VerificationError(format!("Could not verify stripe webhook: {}", e)))
}
