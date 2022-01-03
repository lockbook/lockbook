use crate::file_index_repo::{GetStripeCustomerIdError, FREE_TIER_SIZE};
use crate::ServerError::{ClientError, InternalError};
use crate::{file_index_repo, RequestContext, ServerError};
use lockbook_models::api::{
    AccountTier, CreditCardInfo, GetRegisteredCreditCardsError, RegisterCreditCardError,
    RegisterCreditCardRequest, RegisterCreditCardResponse, RemoveCreditCardError,
    RemoveCreditCardRequest, SwitchAccountTierError, SwitchAccountTierRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static PAYMENT_METHODS_ENDPOINT: &str = "/payment_methods";
static DETACH_ENDPOINT: &str = "/detach";
static ATTACH_ENDPOINT: &str = "/attach";
static CUSTOMER_ENDPOINT: &str = "/customers";
static SUBSCRIPTIONS_ENDPOINT: &str = "/subscriptions";
static SETUP_INTENTS_ENDPOINT: &str = "/setup_intents";

#[derive(Serialize, Deserialize)]
struct BasicStripeResponse {
    id: String,
}

#[derive(Serialize, Deserialize)]
struct SetupIntentStripeResponse {
    status: String,
}

#[derive(Serialize, Deserialize)]
struct PaymentMethodStripeResponse {
    id: String,
    card: PaymentMethodCard,
}

#[derive(Serialize, Deserialize)]
struct PaymentMethodCard {
    last4: String,
}

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

    let customer_id = match file_index_repo::get_stripe_customer_id(
        &mut transaction,
        &context.public_key,
    )
    .await
    {
        Ok(customer_id) => customer_id,
        Err(e) => match e {
            GetStripeCustomerIdError::NotAStripeCustomer => {
                let customer_id = server_state
                    .stripe_client
                    .post(format!("{}{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT))
                    .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
                    .send()
                    .await
                    .map_err(|e| InternalError(format!("Cannot create stripe customer: {}", e)))?
                    .json::<BasicStripeResponse>()
                    .await
                    .map_err(|e| {
                        InternalError(format!(
                            "Cannot parse create stripe customer response: {}",
                            e
                        ))
                    })?
                    .id;

                file_index_repo::attach_stripe_customer_id(
                    &mut transaction,
                    &customer_id,
                    &context.public_key,
                )
                .await
                .map_err(|e| {
                    InternalError(format!("Couldn't add customer id to Postgres: {:?}", e))
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

    let mut payment_method_form = HashMap::new();
    payment_method_form.insert("type", "card");
    payment_method_form.insert("card[number]", request.card_number.as_str());
    payment_method_form.insert("card[exp_month]", request.exp_month.as_str());
    payment_method_form.insert("card[exp_year]", request.exp_year.as_str());
    payment_method_form.insert("card[cvc]", request.cvc.as_str());

    let payment_method_resp = server_state
        .stripe_client
        .post(format!("{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT))
        .form(&payment_method_form)
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
        .send()
        .await
        .map_err(|e| InternalError(format!("Cannot create stripe payment method: {}", e)))?
        .json::<PaymentMethodStripeResponse>()
        .await
        .map_err(|e| {
            InternalError(format!(
                "Cannot parse create stripe payment method response: {}",
                e
            ))
        })?;

    let mut attach_payment_method_form = HashMap::new();
    attach_payment_method_form.insert("customer", customer_id.as_str());

    file_index_repo::add_stripe_payment_method(
        &mut transaction,
        &payment_method_resp.id,
        &customer_id,
        &payment_method_resp.card.last4,
    )
    .await
    .map_err(|e| InternalError(format!("Couldn't add payment method to Postgres: {:?}", e)))?;

    let mut create_setup_intent_form = HashMap::new();
    create_setup_intent_form.insert("customer", customer_id.as_str());
    create_setup_intent_form.insert("payment_method", payment_method_resp.id.as_str());
    create_setup_intent_form.insert("confirm", "true");
    create_setup_intent_form.insert("usage", "on_session");

    let setup_intent_resp = server_state
        .stripe_client
        .post(format!("{}{}", STRIPE_ENDPOINT, SETUP_INTENTS_ENDPOINT))
        .form(&create_setup_intent_form)
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
        .send()
        .await
        .map_err(|e| InternalError(format!("Cannot create stripe setup intent: {}", e)))?
        .json::<SetupIntentStripeResponse>()
        .await
        .map_err(|e| {
            InternalError(format!(
                "Cannot parse create stripe setup intent response: {}",
                e
            ))
        })?
        .status;

    if setup_intent_resp == "succeeded" {
        server_state
            .stripe_client
            .post(format!(
                "{}{}/{}{}",
                STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT, payment_method_resp.id, ATTACH_ENDPOINT
            ))
            .form(&attach_payment_method_form)
            .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
            .send()
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot attach payment method to customer: {:#?}",
                    e
                ))
            })?;

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
            "Unexpected confirmation of stripe setup intent: {}",
            setup_intent_resp
        )))
    }
}

pub async fn remove_credit_card(
    context: RequestContext<'_, RemoveCreditCardRequest>,
) -> Result<(), ServerError<RemoveCreditCardError>> {
    let server_state = context.server_state;

    server_state
        .stripe_client
        .post(format!(
            "{}/{}{}",
            STRIPE_ENDPOINT, context.request.payment_method_id, DETACH_ENDPOINT
        ))
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
        .send()
        .await
        .map_err(|e| InternalError(format!("Cannot remove credit card: {:#?}", e)))?;

    Ok(())
}

pub async fn switch_user_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>,
) -> Result<(), ServerError<SwitchAccountTierError>> {
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
        if let AccountTier::Free = request.account_tier {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }

        let mut create_subscription_form = HashMap::new();
        create_subscription_form.insert("customer", customer_id.as_str()); // TODO: customer id from db
        create_subscription_form.insert(
            "items[0][price]",
            server_state.config.stripe.premium_price_id.as_str(),
        );

        let subscription_id = server_state
            .stripe_client
            .post(format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT))
            .form(&create_subscription_form)
            .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
            .send()
            .await
            .map_err(|e| InternalError(format!("Cannot create stripe subscription: {:?}", e)))?
            .json::<BasicStripeResponse>()
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot parse create stripe subscription response: {:?}",
                    e
                ))
            })?
            .id;

        file_index_repo::add_stripe_subscription(&mut transaction, &customer_id, &subscription_id)
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot add stripe subscription in Postgres: {:?}",
                    e
                ))
            })?;
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

        server_state
            .stripe_client
            .delete(format!(
                "{}{}/{}",
                STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subscription_id
            ))
            .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
            .send()
            .await
            .map_err(|e| InternalError(format!("Cannot cancel stripe subscription: {:?}", e)))?;

        file_index_repo::cancel_stripe_subscription(&mut transaction, &subscription_id)
            .await
            .map_err(|e| {
                InternalError(format!(
                    "Cannot cancel stripe subscription in Postgres: {:?}",
                    e
                ))
            })?;
    }

    Ok(())
}

pub async fn get_registered_credit_cards(
    context: RequestContext<'_, GetRegisteredCreditCardsError>,
) -> Result<Vec<CreditCardInfo>, ServerError<GetRegisteredCreditCardsError>> {
    let mut transaction = match context.server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    file_index_repo::get_all_stripe_credit_card_infos(&mut transaction, &context.public_key)
        .await
        .map_err(|e| InternalError(format!("Cannot get all stripe credit card infos: {:?}", e)))
}
