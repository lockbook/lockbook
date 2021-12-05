use crate::{RequestContext, ServerError};
use lockbook_models::api::{
    RegisterCreditCard, RegisterCreditCardError, RegisterCreditCardResponse,
};
use log::info;
use std::collections::HashMap;

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static CREATE_PAYMENT_ENDPOINT: &str = "/payment_methods";

pub async fn register_for_stripe(
    context: RequestContext<'_, RegisterCreditCard>,
) -> Result<(), ServerError<RegisterCreditCardError>> {
    let (request, server_state) = (&context.request, context.server_state);

    let mut request_to_stripe = HashMap::new();
    request_to_stripe.insert("type", "card");
    request_to_stripe.insert("card[number]", "4242424242424242");
    request_to_stripe.insert("card[exp_month]", "12");
    request_to_stripe.insert("card[exp_year]", "2022");
    request_to_stripe.insert("card[cvc]", "314");

    let resp = server_state
        .stripe_client
        .post(format!("{}{}", STRIPE_ENDPOINT, CREATE_PAYMENT_ENDPOINT))
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None)
        .form(&request_to_stripe)
        .send()
        .await;

    info!("{:#?}", resp);

    Ok(())
}
