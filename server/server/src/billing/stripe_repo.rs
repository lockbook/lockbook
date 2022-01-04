use crate::billing::stripe::{
    BasicStripeResponse, PaymentMethodStripeResponse, SetupIntentStatus, SetupIntentStripeResponse,
    StripeErrorType, StripeResult,
};
use crate::ServerError::{ClientError, InternalError};
use crate::{ServerError, ServerState};
use lockbook_models::api::{
    RegisterCreditCardError, RemoveCreditCardError, SwitchAccountTierError,
};
use reqwest::Method;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static PAYMENT_METHODS_ENDPOINT: &str = "/payment_methods";
static DETACH_ENDPOINT: &str = "/detach";
static ATTACH_ENDPOINT: &str = "/attach";
static CUSTOMER_ENDPOINT: &str = "/customers";
static SUBSCRIPTIONS_ENDPOINT: &str = "/subscriptions";
static SETUP_INTENTS_ENDPOINT: &str = "/setup_intents";

pub async fn create_customer(
    server_state: &ServerState,
) -> Result<String, ServerError<RegisterCreditCardError>> {
    match send_stripe_request::<BasicStripeResponse, RegisterCreditCardError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT),
        Method::POST,
        None,
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp.id),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst creating an account: {:?}",
            e
        ))),
    }
}

pub async fn create_payment_method(
    server_state: &ServerState,
    card_number: &str,
    card_exp_month: &str,
    card_exp_year: &str,
    card_cvc: &str,
) -> Result<PaymentMethodStripeResponse, ServerError<RegisterCreditCardError>> {
    let mut payment_method_form = HashMap::new();
    payment_method_form.insert("type", "card");
    payment_method_form.insert("card[number]", card_number);
    payment_method_form.insert("card[exp_month]", card_exp_month);
    payment_method_form.insert("card[exp_year]", card_exp_year);
    payment_method_form.insert("card[cvc]", card_cvc);

    match send_stripe_request::<PaymentMethodStripeResponse, RegisterCreditCardError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT),
        Method::POST,
        Some(payment_method_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp),
        StripeResult::Err(e) => match e.error.error_type {
            StripeErrorType::CardError => Err(ClientError(
                RegisterCreditCardError::InvalidCreditCardFormat,
            )),
            _ => Err(InternalError(format!(
                "Stripe returned an error whilst creating a payment method: {:?}",
                e
            ))),
        },
    }
}

pub async fn create_setup_intent(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str,
) -> Result<SetupIntentStatus, ServerError<RegisterCreditCardError>> {
    let mut create_setup_intent_form = HashMap::new();
    create_setup_intent_form.insert("customer", customer_id);
    create_setup_intent_form.insert("payment_method", payment_method_id);
    create_setup_intent_form.insert("confirm", "true");
    create_setup_intent_form.insert("usage", "on_session");

    match send_stripe_request::<SetupIntentStripeResponse, RegisterCreditCardError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, SETUP_INTENTS_ENDPOINT),
        Method::POST,
        Some(create_setup_intent_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp.status),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst creating a setup intent: {:?}",
            e
        ))),
    }
}

pub async fn attach_payment_method_to_customer(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str,
) -> Result<(), ServerError<RegisterCreditCardError>> {
    let mut attach_payment_method_form = HashMap::new();
    attach_payment_method_form.insert("customer", customer_id);

    match send_stripe_request::<BasicStripeResponse, RegisterCreditCardError>(
        server_state,
        format!(
            "{}{}/{}{}",
            STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT, payment_method_id, ATTACH_ENDPOINT
        ),
        Method::POST,
        Some(attach_payment_method_form),
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst attaching a payment method to customer: {:?}",
            e
        ))),
    }
}

pub async fn detach_payment_method_from_customer(
    server_state: &ServerState,
    payment_method_id: &str,
) -> Result<(), ServerError<RemoveCreditCardError>> {
    match send_stripe_request::<BasicStripeResponse, RemoveCreditCardError>(
        server_state,
        format!(
            "{}/{}{}",
            STRIPE_ENDPOINT, payment_method_id, DETACH_ENDPOINT
        ),
        Method::POST,
        None,
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst detaching a payment method from a customer: {:?}",
            e
        ))),
    }
}

pub async fn create_subscription(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str
) -> Result<String, ServerError<SwitchAccountTierError>> {
    let mut create_subscription_form = HashMap::new();
    create_subscription_form.insert("customer", customer_id);
    create_subscription_form.insert(
        "items[0][price]",
        server_state.config.stripe.premium_price_id.as_str(),
    );
    create_subscription_form.insert("default_payment_method", payment_method_id);

    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT),
        Method::POST,
        Some(create_subscription_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp.id),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst creating a subscription: {:?}",
            e
        ))),
    }
}

pub async fn delete_subscription(
    server_state: &ServerState,
    subscription_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!(
            "{}{}/{}",
            STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subscription_id
        ),
        Method::DELETE,
        None,
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst deleting a subscription: {:?}",
            e
        ))),
    }
}

async fn send_stripe_request<U: DeserializeOwned, E: Debug>(
    server_state: &ServerState,
    url: String,
    method: Method,
    maybe_form: Option<HashMap<&str, &str>>,
) -> Result<StripeResult<U>, ServerError<E>> {
    let request = server_state
        .stripe_client
        .request(method, &url)
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None);

    if let Some(form) = maybe_form {
        request.form(&form)
    } else {
        request
    }
    .send()
    .await
    .map_err(|e| {
        InternalError(format!(
            "Cannot make stripe request at '{}' due to reqwest error: {:?}",
            url, e
        ))
    })?
    .json::<StripeResult<U>>()
    .await
    .map_err(|e| InternalError(format!("Cannot parse stripe request at '{}': {:?}", url, e)))
}
