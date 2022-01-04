use crate::billing::stripe::SetupIntentStatus;
use crate::billing::stripe_client;
use crate::file_index_repo::{GetStripeCustomerIdError, FREE_TIER_SIZE};
use crate::ServerError::{ClientError, InternalError};
use crate::{file_index_repo, RequestContext, ServerError};
use lockbook_models::api::{
    AccountTier, CreditCardInfo, GetRegisteredCreditCardsError, GetRegisteredCreditCardsRequest,
    GetRegisteredCreditCardsResponse, RegisterCreditCardError, RegisterCreditCardRequest,
    RegisterCreditCardResponse, RemoveCreditCardError, RemoveCreditCardRequest,
    RemoveCreditCardResponse, SwitchAccountTierError, SwitchAccountTierRequest,
    SwitchAccountTierResponse,
};

pub async fn register_credit_card(
    context: RequestContext<'_, RegisterCreditCardRequest>,
) -> Result<RegisterCreditCardResponse, ServerError<RegisterCreditCardError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let payment_method_resp = stripe_client::create_payment_method(
        &server_state,
        &request.card_number,
        &request.exp_month,
        &request.exp_year,
        &request.cvc,
    )
    .await?;

    let customer_id = match file_index_repo::get_stripe_customer_id(
        &mut transaction,
        &context.public_key,
    )
        .await
    {
        Ok(customer_id) => customer_id,
        Err(e) => match e {
            GetStripeCustomerIdError::NotAStripeCustomer => {
                let customer_id = stripe_client::create_customer(&server_state).await?;

                file_index_repo::attach_stripe_customer_id(
                    &mut transaction,
                    &customer_id,
                    &context.public_key,
                )
                    .await
                    .map_err(|e| {
                        InternalError(format!(
                            "Couldn't insert payment method into Postgres: {:?}",
                            e
                        ))
                    })?;

                customer_id
            }
            _ => {
                return Err(InternalError(format!(
                    "Cannot get stripe customer id in Postgres: {:?}",
                    e
                )))
            }
        },
    };

    file_index_repo::add_stripe_payment_method(
        &mut transaction,
        &payment_method_resp.id,
        &customer_id,
        &payment_method_resp.card.last4,
    )
    .await
    .map_err(|e| InternalError(format!("Couldn't add payment method to Postgres: {:?}", e)))?;

    let setup_intent_status =
        stripe_client::create_setup_intent(&server_state, &customer_id, &payment_method_resp.id)
            .await?;

    if let SetupIntentStatus::Succeeded = setup_intent_status {
        stripe_client::attach_payment_method_to_customer(
            &server_state,
            &customer_id,
            &payment_method_resp.id,
        )
        .await?;

        match transaction.commit().await {
            Ok(()) => Ok(RegisterCreditCardResponse {
                credit_card_info: CreditCardInfo {
                    payment_method_id: payment_method_resp.id,
                    last_4_digits: payment_method_resp.card.last4,
                },
            }),
            Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
        }
    } else {
        Err(InternalError(format!(
            "Unexpected confirmation of stripe setup intent: {:?}",
            setup_intent_status
        )))
    }
}

pub async fn remove_credit_card(
    context: RequestContext<'_, RemoveCreditCardRequest>,
) -> Result<RemoveCreditCardResponse, ServerError<RemoveCreditCardError>> {
    let mut transaction = match context.server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    stripe_client::detach_payment_method_from_customer(
        &context.server_state,
        &context.request.payment_method_id,
    )
    .await?;

    file_index_repo::delete_stripe_payment_method(
        &mut transaction,
        &context.request.payment_method_id,
    )
    .await
    .map_err(|e| {
        InternalError(format!(
            "Couldn't delete payment method from Postgres: {:?}",
            e
        ))
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(RemoveCreditCardResponse {}),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
}

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

    let customer_id =
        file_index_repo::get_stripe_customer_id(&mut transaction, &context.public_key)
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot get stripe customer id in Postgres: {:?}",
                    e
                ))
            })?;

    if data_cap == FREE_TIER_SIZE as u64 {
        if let AccountTier::Free = &request.account_tier {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        } else if let AccountTier::Monthly(payment_method_id) = &request.account_tier {
            let subscription_id =
                stripe_client::create_subscription(&server_state, &customer_id, payment_method_id)
                    .await?;

            file_index_repo::add_stripe_subscription(
                &mut transaction,
                &customer_id,
                &subscription_id,
            )
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot add stripe subscription in Postgres: {:?}",
                    e
                ))
            })?;
        }
    } else {
        if data_cap != FREE_TIER_SIZE as u64 {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
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
    }

    match transaction.commit().await {
        Ok(()) => Ok(SwitchAccountTierResponse {}),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_registered_credit_cards(
    context: RequestContext<'_, GetRegisteredCreditCardsRequest>,
) -> Result<GetRegisteredCreditCardsResponse, ServerError<GetRegisteredCreditCardsError>> {
    let mut transaction = match context.server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let credit_card_infos =
        file_index_repo::get_all_stripe_credit_card_infos(&mut transaction, &context.public_key)
            .await
            .map_err(|e| {
                InternalError(format!("Cannot get all stripe credit card infos: {:?}", e))
            })?;

    Ok(GetRegisteredCreditCardsResponse { credit_card_infos })
}
