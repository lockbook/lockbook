use crate::billing::payment_service::StripeWebhookError::{
    InvalidBody, InvalidHeader, VerificationError,
};
use crate::billing::stripe::{
    StripeBillingReason, StripeEventType, StripeMaybeContainer, StripeObjectType, StripeWebhook,
};
use crate::billing::stripe_client;
use crate::file_index_repo::{GetLastStripeCreditCardInfoError, FREE_TIER_SIZE, PAID_TIER_SIZE};
use crate::ServerError::{ClientError, InternalError};
use crate::{file_index_repo, RequestContext, ServerError, ServerState, StripeClientError};
use hmac::{Hmac, Mac};
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, CardChoice, GetLastRegisteredCreditCardError, GetLastRegisteredCreditCardRequest,
    GetLastRegisteredCreditCardResponse, SwitchAccountTierError, SwitchAccountTierRequest,
    SwitchAccountTierResponse,
};
use sha2::Sha256;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

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

    let data_cap = file_index_repo::get_account_data_cap(&mut transaction, &context.public_key)
        .await
        .map_err(|e| InternalError(format!("Cannot get account data cap in Postgres: {:?}", e)))?;

    match (data_cap as i64, &request.account_tier) {
        (FREE_TIER_SIZE, AccountTier::Monthly(card)) => {
            create_subscription(&context.public_key, server_state, &mut transaction, card).await?;
        }
        (FREE_TIER_SIZE, AccountTier::Free) | (_, AccountTier::Monthly(_)) => {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }
        (_, AccountTier::Free) => {
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

            stripe_client::delete_subscription(&server_state, &subscription_id)
                .await
                .map_err(ServerError::<SwitchAccountTierError>::from)?;

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
                FREE_TIER_SIZE,
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
    card: &CardChoice,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let (customer_id, payment_method_id) = match card {
        CardChoice::NewCard {
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
            .await
            .map_err(ServerError::<SwitchAccountTierError>::from)?;

            let customer_id = stripe_client::create_customer(&server_state)
                .await
                .map_err(ServerError::<SwitchAccountTierError>::from)?;

            match file_index_repo::get_last_stripe_credit_card_info(&mut transaction, &public_key)
                .await
            {
                Ok(credit_card_info) => {
                    stripe_client::detach_payment_method_from_customer(
                        &server_state,
                        &credit_card_info.payment_method_id,
                    )
                    .await
                    .map_err(ServerError::<SwitchAccountTierError>::from)?;
                }
                Err(GetLastStripeCreditCardInfoError::NoPaymentInfo) => {}
                Err(e) => {
                    return Err(InternalError(format!(
                        "Cannot get stripe payment method info from Postgres: {:?}",
                        e
                    )))
                }
            }

            stripe_client::attach_payment_method_to_customer(
                &server_state,
                &customer_id,
                &payment_method_resp.id,
            )
            .await
            .map_err(ServerError::<SwitchAccountTierError>::from)?;

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
            .await
            .map_err(ServerError::<SwitchAccountTierError>::from)?;

            (customer_id, payment_method_resp.id)
        }
        CardChoice::OldCard => {
            let old_card =
                file_index_repo::get_last_stripe_credit_card_info(&mut transaction, &public_key)
                    .await
                    .map_err(|e| match e {
                        GetLastStripeCreditCardInfoError::NoPaymentInfo => {
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
                match card {
                    CardChoice::NewCard { .. } => {
                        stripe_client::delete_customer(&server_state, &customer_id)
                            .await
                            .map_err(ServerError::<SwitchAccountTierError>::from)?;
                    }
                    CardChoice::OldCard => {}
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

    file_index_repo::set_account_data_cap(&mut transaction, &public_key, PAID_TIER_SIZE)
        .await
        .map_err(|e| {
            InternalError(format!(
                "Cannot change user data cap to premium data cap: {:?}",
                e
            ))
        })
}

pub async fn get_last_registered_credit_card(
    context: RequestContext<'_, GetLastRegisteredCreditCardRequest>,
) -> Result<GetLastRegisteredCreditCardResponse, ServerError<GetLastRegisteredCreditCardError>> {
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
                GetLastStripeCreditCardInfoError::NoPaymentInfo => {
                    ClientError(GetLastRegisteredCreditCardError::OldCardDoesNotExist)
                }
                _ => InternalError(format!("Cannot get all stripe credit card infos: {:?}", e)),
            })?;

    Ok(GetLastRegisteredCreditCardResponse {
        credit_card_last_4_digits: credit_card.last_4_digits,
    })
}

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
    let webhook = match serde_json::from_slice::<StripeWebhook>(request_body.as_ref()) {
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

    let json = match std::str::from_utf8(&request_body.as_ref()) {
        Ok(json) => json,
        Err(e) => {
            return Err(InvalidBody(format!(
                "Cannot turn stripe webhook body bytes into str: {:?}",
                e
            )))
        }
    };

    let mut sig_header_split = stripe_sig
        .to_str()
        .map_err(|e| {
            InvalidHeader(format!(
                "Cannot turn stripe webhook header into str: {:?}",
                e
            ))
        })?
        .split(",");

    let time = sig_header_split
        .next()
        .ok_or(InvalidHeader(format!(
            "Cannot get the header's time field to verify stripe webhook: {:?}",
            stripe_sig
        )))?
        .split("=")
        .last()
        .ok_or(InvalidHeader(format!(
            "Cannot get the time from the header to verify stripe webhook: {:?}",
            stripe_sig
        )))?;

    let expected_sig = sig_header_split
        .next()
        .ok_or(InvalidHeader(format!(
            "Cannot get the header's expected signature field to verify stripe webhook: {:?}",
            stripe_sig
        )))?
        .split("=")
        .last()
        .ok_or(InvalidHeader(format!(
            "Cannot get the hash from the header to verify stripe webhook: {:?}",
            stripe_sig
        )))?;

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
        .map_err(|e| VerificationError(format!("Could not verify stripe webhook: {}", e)))?;

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
                        FREE_TIER_SIZE,
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
