use async_trait::async_trait;

use tracing::*;

use super::stripe_error::SimplifiedStripeError;

#[async_trait]
pub trait StripeClient: Send + Sync + Clone + 'static {
    async fn create_customer(
        &self, customer_name: String,
    ) -> Result<stripe::Customer, SimplifiedStripeError>;

    /// A hosted page where `customer` subscribes to `price_id`.
    async fn create_checkout_session(
        &self, customer_id: stripe::CustomerId, price_id: &str, success_url: &str, cancel_url: &str,
    ) -> Result<stripe::CheckoutSession, SimplifiedStripeError>;

    /// A hosted page where `customer` manages their subscription.
    async fn create_billing_portal_session(
        &self, customer_id: stripe::CustomerId, return_url: &str,
    ) -> Result<stripe::BillingPortalSession, SimplifiedStripeError>;

    async fn cancel_subscription_at_period_end(
        &self, subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError>;

    async fn get_subscription(
        &self, subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError>;

    /// Every subscription of `customer`, with its customer and default payment method expanded.
    async fn list_subscriptions(
        &self, customer_id: stripe::CustomerId,
    ) -> Result<Vec<stripe::Subscription>, SimplifiedStripeError>;

    /// Expires `customer`'s checkout pages that are still open, so only the newest can be paid.
    async fn expire_open_checkout_sessions(
        &self, customer_id: stripe::CustomerId,
    ) -> Result<(), SimplifiedStripeError>;
}

#[async_trait]
impl StripeClient for stripe::Client {
    async fn create_customer(
        &self, customer_name: String,
    ) -> Result<stripe::Customer, SimplifiedStripeError> {
        let mut customer_params = stripe::CreateCustomer::new();
        customer_params.name = Some(&customer_name);

        let customer = stripe::Customer::create(self, customer_params).await?;
        debug!(?customer, "Created stripe customer");
        Ok(customer)
    }

    async fn create_checkout_session(
        &self, customer_id: stripe::CustomerId, price_id: &str, success_url: &str, cancel_url: &str,
    ) -> Result<stripe::CheckoutSession, SimplifiedStripeError> {
        info!(customer_id = customer_id.as_str(), ?price_id, "Creating stripe checkout session");

        let mut params = stripe::CreateCheckoutSession::new();
        params.mode = Some(stripe::CheckoutSessionMode::Subscription);
        params.customer = Some(customer_id);
        params.line_items = Some(vec![stripe::CreateCheckoutSessionLineItems {
            price: Some(price_id.to_string()),
            quantity: Some(1),
            ..Default::default()
        }]);
        params.success_url = Some(success_url);
        params.cancel_url = Some(cancel_url);
        params.custom_text = Some(stripe::CreateCheckoutSessionCustomText {
            submit: Some(stripe::CreateCheckoutSessionCustomTextSubmit {
                message: "Cancel anytime from Settings in Lockbook.".into(),
            }),
            ..Default::default()
        });

        let session = stripe::CheckoutSession::create(self, params).await?;
        debug!(?session, "Created stripe checkout session");
        Ok(session)
    }

    async fn create_billing_portal_session(
        &self, customer_id: stripe::CustomerId, return_url: &str,
    ) -> Result<stripe::BillingPortalSession, SimplifiedStripeError> {
        info!(customer_id = customer_id.as_str(), "Creating stripe billing portal session");

        let mut params = stripe::CreateBillingPortalSession::new(customer_id);
        params.return_url = Some(return_url);

        let session = stripe::BillingPortalSession::create(self, params).await?;
        debug!(?session, "Created stripe billing portal session");
        Ok(session)
    }

    async fn cancel_subscription_at_period_end(
        &self, subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        info!(
            subscription_id = subscription_id.as_str(),
            "Canceling stripe subscription at period end"
        );

        let params =
            stripe::UpdateSubscription { cancel_at_period_end: Some(true), ..Default::default() };
        let subscription = stripe::Subscription::update(self, subscription_id, params).await?;
        debug!(?subscription, "Scheduled stripe subscription to cancel");
        Ok(subscription)
    }

    async fn get_subscription(
        &self, subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        let subscription = stripe::Subscription::retrieve(self, subscription_id, &[]).await?;
        debug!(?subscription, "Retrieved stripe subscription");
        Ok(subscription)
    }

    async fn list_subscriptions(
        &self, customer_id: stripe::CustomerId,
    ) -> Result<Vec<stripe::Subscription>, SimplifiedStripeError> {
        let params = stripe::ListSubscriptions {
            customer: Some(customer_id),
            status: Some(stripe::SubscriptionStatusFilter::All),
            limit: Some(100),
            expand: &["data.customer", "data.default_payment_method"],
            ..Default::default()
        };
        let subscriptions = stripe::Subscription::list(self, &params).await?;
        debug!(?subscriptions, "Listed stripe subscriptions");
        Ok(subscriptions.data)
    }

    async fn expire_open_checkout_sessions(
        &self, customer_id: stripe::CustomerId,
    ) -> Result<(), SimplifiedStripeError> {
        let params = stripe::ListCheckoutSessions {
            customer: Some(customer_id),
            status: Some(stripe::CheckoutSessionStatus::Open),
            limit: Some(100),
            ..Default::default()
        };
        for session in stripe::CheckoutSession::list(self, &params).await?.data {
            info!(session = session.id.as_str(), "Expiring an older stripe checkout session");
            stripe::CheckoutSession::expire(self, &session.id).await?;
        }
        Ok(())
    }
}
