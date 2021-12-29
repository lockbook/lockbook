use crate::{RequestContext, ServerError, ServerState};
use lockbook_models::api::{RegisterCreditCardRequest, RegisterCreditCardError, RegisterCreditCardResponse, RemoveCreditCardRequest, RemoveCreditCardError, SwitchUserTierRequest, SwitchUserTierError, Tier, GetRegisteredCreditCardsError};
use log::info;
use std::collections::HashMap;
use reqwest::{Client, Error, Response};
use serde_json::Value;
use crate::ServerError::InternalError;

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static PAYMENT_METHODS_ENDPOINT: &str = "/payment_methods";
static DETACH_ENDPOINT: &str = "/detach";
static ATTACH_ENDPOINT: &str = "/attach";
static CUSTOMER_ENDPOINT: &str = "/customers";
static SUBSCRIPTIONS_ENDPOINT: &str = "/subscriptions";

#[derive(Serialize, Deserialize)]
struct StripeResponse {
    id: String,
}
//  Maybe you could log the string response from request and just get rid of the hashmap

pub async fn register_credit_card(
    context: RequestContext<'_, RegisterCreditCardRequest>,
) -> Result<RegisterCreditCardResponse, ServerError<RegisterCreditCardError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut payment_method_form = HashMap::new();
    payment_method_form.insert("type", "card");
    payment_method_form.insert("card[number]", request.card_number.as_str());
    payment_method_form.insert("card[exp_month]", request.exp_month.as_str());
    payment_method_form.insert("card[exp_year]", request.exp_year.as_str());
    payment_method_form.insert("card[cvc]", request.cvc.as_str());

    let payment_method_id = send_stripe_request(
        &server_state,
        format!("{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT),
        StripeRequestType::Post(Some(payment_method_form)))
        .and_then(|resp| resp.json::<StripeResponse>().await)
        .map_err(|_| Err(InternalError("Error creating stripe payment method".to_string())))?
        .id;
    // should never have an error, should probably log this error stuff

    // TODO: Check db if user has a customer id

    // assuming that this user does not have a customer id -----------------------------------------------

    let customer_id = send_stripe_request(
        &server_state,
        format!("{}{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT),
        StripeRequestType::Post(None))
        .and_then(|resp| resp.json::<StripeResponse>().await)
        .map_err(|_| Err(InternalError("Error creating stripe customer".to_string())))?
        .id;

    // Commit this id to the db

    // End of assuming -----------------------------------------------

    let mut attach_payment_method_form = HashMap::new();
    attach_payment_method_form.insert("customer", customer_id.as_str());

    send_stripe_request(
        &server_state,
        format!("{}{}/{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT, payment_method_id, ATTACH_ENDPOINT),
        StripeRequestType::Post(Some(attach_payment_method_form)))
        .map_err(|_| Err(InternalError("Error attaching stripe payment method to customer".to_string())))?;

    Ok(RegisterCreditCardResponse { payment_method_id })
}

pub async fn remove_credit_card(
    context: RequestContext<'_, RemoveCreditCardRequest>
) -> Result<(), ServerError<RemoveCreditCardError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let resp = send_stripe_request(
        &server_state,
        format!("{}/{}{}", STRIPE_ENDPOINT, request.payment_method_id, DETACH_ENDPOINT),
        StripeRequestType::Post(None));

    if let Err(_) = resp {
        return Err(InternalError("Error removing stripe payment method".to_string()));
    }

    Ok(())
}

pub async fn switch_user_tier(
    context: RequestContext<'_, SwitchUserTierRequest>
) -> Result<(), ServerError<SwitchUserTierError>> {
    let (request, server_state) = (&context.request, context.server_state);

    // Check db to retrieve the current tier, and compare it to the requested.
    // If the requested tier == current tier, return error

    // If the user does not have a client (as retrieved from DB), return an error

    let (price_id, payment_method_id) = match &request.tier {
        Tier::Monthly(payment_method_id) => {
            (server_state.config.stripe.monthly_sub_price_id.as_str(), payment_method_id)
        }
        Tier::Yearly(payment_method_id) => {
            (server_state.config.stripe.yearly_sub_price_id.as_str(), payment_method_id)
        }
        Tier::Free => {
            // Retrieve subscription id from DB or list out a customer's subscription and get the one
            send_stripe_request(
                &server_state,
                format!("{}{}/{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subcription_id),
                StripeRequestType::Delete)
                .map_err(|_| Err(InternalError("Error canceling stripe subscription".to_string())))?;


            return Ok(());
        }
    };

    if let Tier::Free = request.tier { // TODO: this is supposed to be the original tier that is retrieved from the db
        let mut create_subscription_form = HashMap::new();
        create_subscription_form.insert("customer", "CUSTOMER ID"); // TODO: customer id from db
        create_subscription_form.insert("items[0][price]", price_id);

        let subscription_id = send_stripe_request(
            &server_state,
            format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT),
            StripeRequestType::Post(Some(create_subscription_form)))
            .and_then(|resp| resp.json::<StripeResponse>().await)
            .map_err(|_| Err(InternalError("Error creating stripe subscription".to_string())))?
            .id;

        // Commit subscription id to db (or maybe not, retrieve it every request from stripe?)
    } else {
        // Retrieve subscription id from DB

        let mut update_subscription_form = HashMap::new();
        update_subscription_form.insert("items[price]", price_id);

        send_stripe_request(
            &server_state,
            format!("{}{}/{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subscription_id),
            StripeRequestType::Post(Some(update_subscription_form)))
            .map_err(|_| Err(InternalError("Error attaching stripe payment method to customer".to_string())))?;
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct StripeGetRegisteredCreditCardsResponse {
    data: Vec<StripeResponse>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

pub async fn get_registered_credit_cards(
    context: RequestContext<'_, SwitchUserTierRequest>
) -> Result<(), ServerError<GetRegisteredCreditCardsError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let subscription_id = send_stripe_request(
        &server_state,
        format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT),
        StripeRequestType::Post(Some(create_subscription_form)))
        .and_then(|resp| resp.json::<StripeResponse>().await)
        .map_err(|_| Err(InternalError("Error creating stripe subscription".to_string())))?
        .id;
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
                .post(url); // Subscription id from form


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
