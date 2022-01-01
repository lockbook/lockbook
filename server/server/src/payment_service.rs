use crate::{file_index_repo, RequestContext, ServerError, ServerState};
use lockbook_models::api::{RegisterCreditCardRequest, RegisterCreditCardError, RegisterCreditCardResponse, RemoveCreditCardRequest, RemoveCreditCardError, SwitchAccountTierRequest, SwitchAccountTierError, AccountTier, GetRegisteredCreditCardsError, CreditCardInfo};
use log::info;
use std::collections::HashMap;
use reqwest::{Client, Error, Response};
use serde_json::Value;
use warp::hyper::body::HttpBody;
use crate::file_index_repo::{AttachStripeCustomerIdError, FREE_TIER_SIZE, GetStripeCustomerIdError};
use crate::ServerError::{ClientError, InternalError};

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static PAYMENT_METHODS_ENDPOINT: &str = "/payment_methods";
static DETACH_ENDPOINT: &str = "/detach";
static ATTACH_ENDPOINT: &str = "/attach";
static CUSTOMER_ENDPOINT: &str = "/customers";
static SUBSCRIPTIONS_ENDPOINT: &str = "/subscriptions";
static SETUP_INTENTS_ENDPOINT: &str = "/setup_intents";

#[derive(Serialize)]
struct BasicStripeResponse {
    id: String,
}

#[derive(Serialize)]
struct SetupIntentStripeResponse {
    status: String,
}

#[derive(Serialize)]
struct PaymentMethodStripeResponse {
    id: String,
    card: PaymentMethodCard
}

#[derive(Serialize)]
struct PaymentMethodCard {
    last4: String
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

    let customer_id = match file_index_repo::get_stripe_customer_id(&mut transaction, &context.public_key).await {
        Ok(customer_id) => customer_id,
        Err(e) => match e {
            GetStripeCustomerIdError::NotAStripeCustomer => {
                let customer_id = send_stripe_request(
                    &server_state,
                    format!("{}{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT),
                    StripeRequestType::Post(None))
                    .and_then(|resp| resp.json::<BasicStripeResponse>().await)
                    .map_err(|e| Err(InternalError(format!("Cannot create stripe customer: {}", e))))?
                    .id;

                file_index_repo::attach_stripe_customer_id(&mut transaction, &customer_id, &context.public_key).map_err(|e| InternalError(format!("Couldn't add customer id to Postgres: {:?}", e)))?;

                customer_id
            }
            _ => return Err(InternalError(format!("Cannot get stripe customer id in Postgres: {:?}", e)))
        }
    };

    let mut payment_method_form = HashMap::new();
    payment_method_form.insert("type", "card");
    payment_method_form.insert("card[number]", request.card_number.as_str());
    payment_method_form.insert("card[exp_month]", request.exp_month.as_str());
    payment_method_form.insert("card[exp_year]", request.exp_year.as_str());
    payment_method_form.insert("card[cvc]", request.cvc.as_str());

    let payment_method_resp = send_stripe_request(
        &server_state,
        format!("{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT),
        StripeRequestType::Post(Some(payment_method_form)))
        .and_then(|resp| resp.json::<PaymentMethodStripeResponse>().await)
        .map_err(|e| Err(InternalError(format!("Cannot create stripe customer: {}", e))))?;

    let mut attach_payment_method_form = HashMap::new();
    attach_payment_method_form.insert("customer", customer_id.as_str());

    file_index_repo::add_stripe_payment_method(&mut transaction, &payment_method_resp.id, &customer_id, &payment_method_resp.card.last4).map_err(|e| InternalError(format!("Couldn't add payment method to Postgres: {:?}", e)))?;

    let mut create_setup_intent_form = HashMap::new();
    create_setup_intent_form.insert("customer", customer_id.as_str());
    create_setup_intent_form.insert("payment_method", payment_method_resp.id.as_str());
    create_setup_intent_form.insert("confirm", "true");
    create_setup_intent_form.insert("usage", "on_session");

    let setup_intent_resp = send_stripe_request(
        &server_state,
        format!("{}{}", STRIPE_ENDPOINT, SETUP_INTENTS_ENDPOINT),
        StripeRequestType::Post(Some(create_setup_intent_form)))
        .and_then(|resp| resp.json::<SetupIntentStripeResponse>().await)
        .map_err(|e| Err(InternalError(format!("Cannot create stripe setup intent: {}", e))))?
        .status;

    if setup_intent_resp == "succeeded" {
        send_stripe_request(
            &server_state,
            format!("{}{}/{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT, payment_method_resp.id, ATTACH_ENDPOINT),
            StripeRequestType::Post(Some(attach_payment_method_form)))
            .map_err(|e| Err(InternalError(format!("Cannot attach payment method to customer: {:#?}", e))))?;

        match transaction.commit().await {
            Ok(()) => Ok(RegisterCreditCardResponse { payment_method_id: payment_method_resp.id, last_4:payment_method_resp.card.last4 }),
            Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
        }
    } else {
        Err(InternalError(format!("Unexpected confirmation of stripe setup intent: {}", setup_intent_resp)));
    }
}

pub async fn remove_credit_card(
    context: RequestContext<'_, RemoveCreditCardRequest>
) -> Result<(), ServerError<RemoveCreditCardError>> {
   send_stripe_request(
        &context.server_state,
        format!("{}/{}{}", STRIPE_ENDPOINT, context.request.payment_method_id, DETACH_ENDPOINT),
        StripeRequestType::Post(None))
       .map_err(|e| InternalError(format!("Cannot remove credit card: {:#?}", e)));

    Ok(())
}

pub async fn switch_user_tier(
    context: RequestContext<'_, SwitchAccountTierRequest>
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let data_cap = file_index_repo::get_account_data_cap(&mut transaction, &context.public_key).map_err(|e| Err(InternalError(format!("Cannot get account data cap in Postgres: {:?}", e))))?;

    let customer_id = file_index_repo::get_stripe_customer_id(&mut transaction, &context.public_key).map_err(|e| Err(InternalError(format!("Cannot get stripe customer id in Postgres: {:?}", e))))?;

    if data_cap == FREE_TIER_SIZE {
        if let AccountTier::Free = request.account_tier {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }

        let mut create_subscription_form = HashMap::new();
        create_subscription_form.insert("customer", "CUSTOMER ID"); // TODO: customer id from db
        create_subscription_form.insert("items[0][price]", price_id);

        let subscription_id = send_stripe_request(
            &server_state,
            format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT),
            StripeRequestType::Post(Some(create_subscription_form)))
            .and_then(|resp| resp.json::<BasicStripeResponse>().await)
            .map_err(|_| Err(InternalError("Error creating stripe subscription".to_string())))?
            .id;

        file_index_repo::add_stripe_subscription(&mut transaction, &customer_id, &subscription_id).map_err(|e| InternalError(format!("Cannot add stripe subscription in Postgres: {:?}", e)))?;
    } else {
        if data_cap != FREE_TIER_SIZE {
            return Err(ClientError(SwitchAccountTierError::NewTierIsOldTier));
        }

        let subscription_id = file_index_repo::get_active_stripe_subscription_id(&mut transaction, &context.public_key).map_err(|e| InternalError(format!("Cannot retrieve stripe subscription in Postgres: {:?}", e)))?;

        send_stripe_request(
            &server_state,
            format!("{}{}/{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subscription_id),
            StripeRequestType::Delete)
            .map_err(|_| Err(InternalError("Error canceling stripe subscription".to_string())))?;

        file_index_repo::cancel_stripe_subscription(&mut transaction, &subscription_id).map_err(|e| InternalError(format!("Cannot cancel stripe subscription in Postgres: {:?}", e)))?;
    }

    Ok(())
}

pub async fn get_registered_credit_cards(
    context: RequestContext<'_, GetRegisteredCreditCardsError>
) -> Result<Vec<CreditCardInfo>, ServerError<GetRegisteredCreditCardsError>> {
    let mut transaction = match context.server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    file_index_repo::get_all_stripe_credit_card_infos(&mut transaction, &context.server_state.public_key).map_err(|e| InternalError(format!("Cannot get all stripe credit card infos: {:?}", e)))
}

pub enum StripeRequestType<'a> {
    Post(Option<HashMap<&'a str, &'a str>>),
    Get(Option<HashMap<&'a str, &'a str>>),
    Delete,
}

fn send_stripe_request(server_state: &ServerState, url: String, request_type: StripeRequestType) -> Result<Response, reqwest::Error> {
    match request_type {
        StripeRequestType::Post(maybe_form) => {
            let request = server_state
                .stripe_client
                .post(url);


            if let Some(form) = maybe_form {
                request.form(&form);
            }

            request
        }
        StripeRequestType::Delete => {
            server_state
                .stripe_client
                .delete(url)
        }
        StripeRequestType::Get(maybe_form) => {
            let request = server_state
                .stripe_client
                .get(url);


            if let Some(form) = maybe_form {
                request.form(&form);
            }

            request

        }
    }
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
        .send()
        .await
}
