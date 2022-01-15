use crate::billing::stripe::{StripeEventType, StripeObjectType, StripeWebhook};
use crate::billing::stripe_client;
use crate::file_index_repo::{GetLastStripeCreditCardInfoError, UpdateStripeSubscriptionPeriodEnd, FREE_TIER_SIZE, PAID_TIER_SIZE, SetDataCapWithStripeCustomerIdError};
use crate::ServerError::{ClientError, InternalError};
use crate::{file_index_repo, RequestContext, ServerError, ServerState, StripeClientError};
use libsecp256k1::PublicKey;
use lockbook_models::api::{
    AccountTier, CardChoice, GetLastRegisteredCreditCardError, GetLastRegisteredCreditCardRequest,
    GetLastRegisteredCreditCardResponse, SwitchAccountTierError, SwitchAccountTierRequest,
    SwitchAccountTierResponse,
};
use log::error;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use warp::http::StatusCode;
use warp::hyper::body::Bytes;
use warp::reply::WithStatus;

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

pub async fn stripe_webhook(state: &Arc<ServerState>, request: Bytes) -> WithStatus<String> {
    let webhook = match serde_json::from_slice::<StripeWebhook>(request.as_ref()) {
        Ok(webhook) => webhook,
        Err(_) => return warp::reply::with_status("".to_string(), StatusCode::OK),
    };

    let mut transaction = match state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("Cannot begin transaction: {:?}", e);
            return warp::reply::with_status("".to_string(), StatusCode::OK);
        }
    };

    match (webhook.event_type, webhook.data.object) {
        (StripeEventType::InvoicePaymentFailed, StripeObjectType::Invoice(invoice)) => {
            if let Err(e) = file_index_repo::set_data_cap_with_stripe_customer_id(&mut transaction, &invoice.customer_id).await {
                error!(
                    "Cannot change data cap with customer id in Postgres: {:?}",
                    invoice
                )
            }
        }
        (StripeEventType::InvoicePaid, StripeObjectType::Invoice(partial_invoice)) => {
            match stripe_client::retrieve_invoice(&state, &partial_invoice.id).await {
                Ok(invoice) => match invoice.subscription {
                    None => error!(
                        "There should be a subscription tied to this invoice: {:?}",
                        invoice
                    ),
                    Some(subscription) => {
                        if let Err(e) = file_index_repo::update_stripe_subscription_period_end(
                            &mut transaction,
                            &subscription.id,
                            subscription.current_period_end,
                        )
                        .await
                        {
                            error!("Couldn't update subscription period in Postgres: {:?}", e)
                        }
                    }
                },
                Err(e) => {}
            }
        }
    };

    if let Err(e) = transaction.commit().await {
        error!("Cannot commit transaction: {:?}", e)
    }

    warp::reply::with_status("".to_string(), StatusCode::OK)
}
