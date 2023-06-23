use async_trait::async_trait;

use tracing::*;

use super::stripe_error::SimplifiedStripeError;

#[async_trait]
pub trait StripeClient {
    async fn create_customer(
        &self, customer_name: String, payment_method_id: stripe::PaymentMethodId,
    ) -> Result<stripe::Customer, SimplifiedStripeError>
    where
        Self: Sized;
}

// #[async_trait]
// impl StripeClient for stripe::Client {
//     async fn create_customer(
//         &self, customer_name: String, payment_method_id: stripe::PaymentMethodId,
//     ) -> Result<stripe::Customer, SimplifiedStripeError> {
//         {
//             let payment_method_id = payment_method_id.as_str();
//             info!(?payment_method_id, "Creating stripe customer");
//         }
//
//         let mut customer_params = stripe::CreateCustomer::new();
//         customer_params.payment_method = Some(payment_method_id);
//         customer_params.name = Some(customer_name);
//
//         let customer = stripe::Customer::create(self, customer_params)
//             .await
//             .map_err(SimplifiedStripeError::from)?;
//
//         debug!(?customer, "Created stripe customer");
//
//         Ok(customer)
//     }
// }

pub async fn create_customer(
    stripe_client: &stripe::Client, customer_name: &str, payment_method_id: stripe::PaymentMethodId,
) -> Result<stripe::Customer, SimplifiedStripeError> {
    {
        let payment_method_id = payment_method_id.as_str();
        info!(?payment_method_id, "Creating stripe customer");
    }

    let mut customer_params = stripe::CreateCustomer::new();
    customer_params.payment_method = Some(payment_method_id);
    customer_params.name = Some(customer_name);

    let customer = stripe::Customer::create(stripe_client, customer_params)
        .await
        .map_err(SimplifiedStripeError::from)?;

    debug!(?customer, "Created stripe customer");

    Ok(customer)
}

pub async fn create_payment_method(
    stripe_client: &stripe::Client, card_number: &str, exp_month: i32, exp_year: i32, cvc: &str,
) -> Result<stripe::PaymentMethod, SimplifiedStripeError> {
    let mut payment_method_params = stripe::CreatePaymentMethod::new();
    payment_method_params.type_ = Some(stripe::PaymentMethodTypeFilter::Card);
    payment_method_params.card =
        Some(stripe::CreatePaymentMethodCardUnion::CardDetailsParams(stripe::CardDetailsParams {
            cvc: Some(cvc.to_string()),
            exp_month,
            exp_year,
            number: card_number.to_string(),
        }));

    let payment_method = stripe::PaymentMethod::create(stripe_client, payment_method_params)
        .await
        .map_err(SimplifiedStripeError::from)?;

    debug!(?payment_method, "Created stripe payment method");

    Ok(payment_method)
}

pub async fn create_setup_intent(
    stripe_client: &stripe::Client, customer_id: stripe::CustomerId,
    payment_method_id: stripe::PaymentMethodId,
) -> Result<stripe::SetupIntent, SimplifiedStripeError> {
    {
        let customer_id = customer_id.as_str();
        let payment_method_id = payment_method_id.as_str();
        info!(?customer_id, ?payment_method_id, "Creating stripe setup intent");
    }

    let mut setup_intent_params = stripe::CreateSetupIntent::new();
    setup_intent_params.customer = Some(customer_id);
    setup_intent_params.payment_method = Some(payment_method_id);
    setup_intent_params.confirm = Some(true);

    let setup_intent = stripe::SetupIntent::create(stripe_client, setup_intent_params).await?;

    debug!(?setup_intent, "Created stripe setup intent");

    match setup_intent.status {
        stripe::SetupIntentStatus::Succeeded => Ok(setup_intent),
        _ => Err(SimplifiedStripeError::Other(format!(
            "Unexpected intent response status: {:?}",
            setup_intent.status
        ))),
    }
}

pub async fn create_subscription(
    stripe_client: &stripe::Client, customer_id: stripe::CustomerId, payment_method_id: &str,
    price_id: &str,
) -> Result<stripe::Subscription, SimplifiedStripeError> {
    {
        let customer_id = customer_id.as_str();
        info!(?customer_id, ?payment_method_id, "Creating stripe subscription");
    }

    let mut subscription_params = stripe::CreateSubscription::new(customer_id);
    let mut subscription_item_params = stripe::CreateSubscriptionItems::new();
    subscription_item_params.price = Some(price_id.to_string());

    subscription_params.default_payment_method = Some(payment_method_id);
    subscription_params.items = Some(vec![subscription_item_params]);
    subscription_params.expand = &["latest_invoice", "latest_invoice.payment_intent"];

    let subscription = stripe::Subscription::create(stripe_client, subscription_params).await?;

    debug!(?subscription, "Created stripe subscription");

    match subscription.status {
        stripe::SubscriptionStatus::Active => Ok(subscription),
        stripe::SubscriptionStatus::Incomplete => match subscription.latest_invoice.as_ref().ok_or_else(|| SimplifiedStripeError::Other(format!("There is no latest invoice for a subscription: {:?}", subscription)))? {
            stripe::Expandable::Id(id) => Err(SimplifiedStripeError::Other(format!("Latest invoice was expanded yet returned an id: {:?}", id))),
            stripe::Expandable::Object(invoice) => match invoice.payment_intent.as_ref().ok_or_else(|| SimplifiedStripeError::Other(format!("No payment intent for latest subscription: {:?}", subscription)))? {
                stripe::Expandable::Id(id) => Err(SimplifiedStripeError::Other(format!("Payment intent expanded yet returned an id: {:?}", id))),
                stripe::Expandable::Object(payment_intent) => match payment_intent.status {
                    stripe::PaymentIntentStatus::RequiresPaymentMethod => Err(SimplifiedStripeError::CardDecline),
                    stripe::PaymentIntentStatus::RequiresAction => Err(SimplifiedStripeError::Other(format!("Payment intent requires additional action to be completed. This is unimplemented. subscription_resp: {:?}", subscription))),
                    _ => Err(SimplifiedStripeError::Other(format!("Unexpected payment intent failure status: {:?}", subscription))),
                }
            }
        }
        _ => Err(SimplifiedStripeError::Other(format!("Unexpected subscription response: {:?}", subscription)))
    }
}

pub async fn detach_payment_method_from_customer(
    stripe_client: &stripe::Client, payment_method_id: &stripe::PaymentMethodId,
) -> Result<(), SimplifiedStripeError> {
    {
        let payment_method_id = payment_method_id.as_str();
        info!(?payment_method_id, "Detaching stripe payment method");
    }

    let payment_method = stripe::PaymentMethod::detach(stripe_client, payment_method_id).await?;

    debug!(?payment_method, "Detached stripe payment method");

    Ok(())
}

pub async fn cancel_subscription(
    stripe_client: &stripe::Client, subscription_id: &stripe::SubscriptionId,
) -> Result<(), SimplifiedStripeError> {
    {
        let subscription_id = subscription_id.as_str();
        info!(?subscription_id, "Cancelling stripe subscription");
    }

    let subscription = stripe::Subscription::cancel(
        stripe_client,
        subscription_id,
        stripe::CancelSubscription::default(),
    )
    .await?;

    debug!(?subscription, "Canceled stripe subscription");

    Ok(())
}

pub async fn get_subscription(
    stripe_client: &stripe::Client, subscription_id: &stripe::SubscriptionId,
) -> Result<stripe::Subscription, SimplifiedStripeError> {
    {
        let subscription_id = subscription_id.as_str();
        info!(?subscription_id, "Retrieving stripe subscription");
    }

    let subscription = stripe::Subscription::retrieve(stripe_client, subscription_id, &[]).await?;

    debug!(?subscription, "Retrieved stripe subscription");

    Ok(subscription)
}

const EXPAND_INVOICE_DETAILS: &[&str] = &["subscription"];

pub async fn retrieve_invoice(
    stripe_client: &stripe::Client, invoice_id: &stripe::InvoiceId,
) -> Result<stripe::Invoice, SimplifiedStripeError> {
    {
        let invoice_id = invoice_id.as_str();
        info!(?invoice_id, "Getting stripe invoice");
    }

    let invoice = stripe::Invoice::retrieve(stripe_client, invoice_id, EXPAND_INVOICE_DETAILS)
        .await
        .map_err(SimplifiedStripeError::from)?;

    debug!(?invoice, "Retrieved stripe invoice");

    Ok(invoice)
}
