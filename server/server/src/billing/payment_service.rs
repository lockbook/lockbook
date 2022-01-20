use crate::billing::payment_service::StripeWebhookError::{
    InvalidBody, InvalidHeader, VerificationError,
};
use crate::billing::stripe_client;
use crate::billing::stripe_model::{
    StripeBillingReason, StripeEventType, StripeMaybeContainer, StripeObjectType,
    StripeWebhookResponse,
};
use crate::ServerError::{ClientError, InternalError};
use hmac::{Hmac, Mac};
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, GetCreditCardError, GetCreditCardRequest, GetCreditCardResponse, PaymentMethod,
    SwitchAccountTierError, SwitchAccountTierRequest, SwitchAccountTierResponse,
};
use sha2::Sha256;
use std::sync::Arc;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;
use crate::{account_service, FREE_TIER_USAGE_SIZE, MONTHLY_TIER_USAGE_SIZE, RequestContext, ServerError, ServerState, StripeClientError};

pub async fn switch_account_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>,
) -> Result<SwitchAccountTierResponse, ServerError<SwitchAccountTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let data_cap = account_service::get_data_cap(&server_state.index_db_pool, &context.public_key).await?;

    match (data_cap as i64, &request.account_tier) {
        (FREE_TIER_USAGE_SIZE, AccountTier::Monthly(card)) => {
            create_subscription(&context.public_key, server_state, &mut transaction, card).await?;
        }
        (FREE_TIER_USAGE_SIZE, AccountTier::Free) | (_, AccountTier::Monthly(_)) => {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }
        (_, AccountTier::Free) => {
            let current_usage: u64 =
                file_index_repo::get_file_usages(&mut transaction, &context.public_key)
                    .await
                    .map_err(|e| InternalError(format!("Cannot get user's usage: {:?}", e)))?
                    .into_iter()
                    .map(|usage| usage.size_bytes)
                    .sum();

            if current_usage > FREE_TIER_USAGE_SIZE {
                return Err(ClientError(
                    SwitchAccountTierError::CurrentUsageIsMoreThanNewTier,
                ));
            }

            let subscription_id = file_index_repo::get_active_stripe_subscription_id(
                &mut transaction,
                &context.public_key,
            )
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot retrieve stripe subscription in Postgres: {:?}",
                    e
                ))
            })?;

            stripe_client::delete_subscription(&server_state, &subscription_id).await?;

            file_index_repo::cancel_stripe_subscription(&mut transaction, &subscription_id)
                .await
                .map_err(|e| {
                    InternalError(format!(
                        "Cannot cancel stripe subscription in Postgres: {:?}",
                        e
                    ))
                })?;

            file_index_repo::set_account_data_cap(
                &mut transaction,
                &context.public_key,
                FREE_TIER_USAGE_SIZE,
            )
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot change user data cap to free data cap: {:?}",
                    e
                ))
            })?;
        }
    }

    match transaction.commit().await {
        Ok(()) => Ok(SwitchAccountTierResponse {}),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
}

async fn create_subscription(
    public_key: &PublicKey,
    server_state: &ServerState,
    mut transaction: &mut Transaction<'_, Postgres>,
    payment_method: &PaymentMethod,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let (customer_id, payment_method_id) = match payment_method {
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

            let customer_id =
                stripe_client::create_customer(&server_state, &payment_method_resp.id)
                    .await?
                    .id;

            match file_index_repo::get_last_stripe_credit_card_info(&mut transaction, &public_key)
                .await
            {
                Ok(credit_card_info) => {
                    stripe_client::detach_payment_method_from_customer(
                        &server_state,
                        &credit_card_info.payment_method_id,
                    )
                    .await?;
                }
                Err(GetCreditCardError::NoPaymentInfo) => {}
                Err(e) => {
                    return Err(InternalError(format!(
                        "Cannot get stripe payment method info from Postgres: {:?}",
                        e
                    )))
                }
            }

            file_index_repo::attach_stripe_customer_id(&mut transaction, &customer_id, &public_key)
                .await
                .map_err(|e| {
                    InternalError(format!(
                        "Couldn't insert payment method into Postgres: {:?}",
                        e
                    ))
                })?;

            file_index_repo::add_stripe_payment_method(
                &mut transaction,
                &payment_method_resp.id,
                &customer_id,
                &payment_method_resp.card.last4,
            )
            .await
            .map_err(|e| {
                InternalError(format!("Couldn't add payment method to Postgres: {:?}", e))
            })?;

            stripe_client::create_setup_intent(
                &server_state,
                &customer_id,
                &payment_method_resp.id,
            )
            .await?;

            (customer_id, payment_method_resp.id)
        }
        PaymentMethod::OldCard => {
            let old_card =
                file_index_repo::get_last_stripe_credit_card_info(&mut transaction, &public_key)
                    .await
                    .map_err(|e| match e {
                        GetCreditCardError::NoPaymentInfo => {
                            ClientError(SwitchAccountTierError::OldCardDoesNotExist)
                        }
                        _ => InternalError(format!(
                            "Cannot get stripe payment method info from Postgres: {:?}",
                            e
                        )),
                    })?;

            let customer_id =
                file_index_repo::get_stripe_customer_id(&mut transaction, &public_key)
                    .await
                    .map_err(|e| {
                        InternalError(format!(
                            "Cannot get stripe customer id from Postgres: {:?}",
                            e
                        ))
                    })?;

            (customer_id, old_card.payment_method_id)
        }
    };

    let subscription_resp =
        match stripe_client::create_subscription(&server_state, &customer_id, &payment_method_id)
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

    file_index_repo::add_stripe_subscription(
        &mut transaction,
        &customer_id,
        &subscription_resp.id,
        subscription_resp.current_period_end,
    )
    .await
    .map_err(|e| {
        InternalError(format!(
            "Cannot add stripe subscription in Postgres: {:?}",
            e
        ))
    })?;

    file_index_repo::set_account_data_cap(&mut transaction, &public_key, MONTHLY_TIER_USAGE_SIZE)
        .await
        .map_err(|e| {
            InternalError(format!(
                "Cannot change user data cap to premium data cap: {:?}",
                e
            ))
        })
}

pub async fn get_credit_card(
    context: RequestContext<'_, GetCreditCardRequest>,
) -> Result<GetCreditCardResponse, ServerError<GetCreditCardError>> {
    let mut transaction = match context.server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let credit_card =
        file_index_repo::get_last_stripe_credit_card_info(&mut transaction, &context.public_key)
            .await
            .map_err(|e| match e {
                GetCreditCardError::NoPaymentInfo => {
                    ClientError(GetCreditCardError::OldCardDoesNotExist)
                }
                _ => InternalError(format!("Cannot get all stripe credit card infos: {:?}", e)),
            })?;

    Ok(GetCreditCardResponse {
        credit_card_last_4_digits: credit_card.last_4_digits,
    })
}

#[derive(Debug)]
pub enum StripeWebhookError {
    VerificationError(String),
    InvalidHeader(String),
    InvalidBody(String),
    InternalError(String),
}

pub async fn stripe_webhooks(
    state: &Arc<ServerState>,
    request_body: Bytes,
    stripe_sig: HeaderValue,
) -> Result<(), StripeWebhookError> {
    let webhook = match serde_json::from_slice::<StripeWebhookResponse>(request_body.as_ref()) {
        Ok(webhook) => webhook,
        Err(e) => {
            return Err(InvalidHeader(format!(
                "Cannot deserialize stripe webhook body: {:?}",
                e
            )))
        }
    };

    let event_type = match webhook.event_type {
        StripeMaybeContainer::Expected(ref known) => known,
        StripeMaybeContainer::Unexpected(_) => return Ok(()),
    };

    verify_stripe_webhook(state, &request_body, stripe_sig)?;

    let mut transaction = match state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(StripeWebhookError::InternalError(format!(
                "Cannot begin transaction: {:?}",
                e
            )))
        }
    };

    match (event_type, &webhook.data.object) {
        (StripeEventType::InvoicePaymentFailed, StripeObjectType::Invoice(invoice)) => {
            match invoice.billing_reason {
                StripeBillingReason::SubscriptionCycle => {
                    file_index_repo::set_data_cap_with_stripe_customer_id(
                        &mut transaction,
                        &invoice.customer_id,
                        FREE_TIER_USAGE_SIZE,
                    )
                    .await
                    .map_err(|e| {
                        StripeWebhookError::InternalError(format!(
                            "Cannot change data cap with customer id in Postgres: {:?}",
                            e
                        ))
                    })?
                }
                _ => return Ok(()),
            }
        }
        (StripeEventType::InvoicePaid, StripeObjectType::Invoice(partial_invoice)) => {
            match partial_invoice.billing_reason {
                StripeBillingReason::SubscriptionCycle => {
                    let invoice = stripe_client::retrieve_invoice(&state, &partial_invoice.id)
                        .await
                        .map_err(|e| {
                            StripeWebhookError::InternalError(format!(
                                "Cannot retrieve expanded invoice: {:?}",
                                e
                            ))
                        })?;

                    let subscription = match invoice.subscription {
                        StripeMaybeContainer::Unexpected(_) => {
                            return Err(StripeWebhookError::InternalError(format!(
                                "There should be a subscription tied to this invoice: {:?}",
                                invoice
                            )))
                        }
                        StripeMaybeContainer::Expected(subscription) => subscription,
                    };

                    file_index_repo::update_stripe_subscription_period_end(
                        &mut transaction,
                        &subscription.id,
                        subscription.current_period_end,
                    )
                    .await
                    .map_err(|e| {
                        StripeWebhookError::InternalError(format!(
                            "Couldn't update subscription period in Postgres: {:?}",
                            e
                        ))
                    })?;
                }
                _ => return Ok(()),
            }
        }
        (_, StripeObjectType::Unmatched(_)) => {
            return Err(StripeWebhookError::InternalError(format!(
                "Unmatched webhook: {:?}",
                webhook
            )))
        }
    };

    match transaction.commit().await {
        Ok(_) => Ok(()),
        Err(e) => Err(StripeWebhookError::InternalError(format!(
            "Couldn't update subscription period in Postgres: {:?}",
            e
        ))),
    }
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
