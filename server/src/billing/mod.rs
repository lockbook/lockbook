pub mod app_store_client;
pub mod app_store_model;
pub mod app_store_service;
pub mod billing_model;
pub mod billing_service;
pub mod google_play_client;
pub mod google_play_model;
pub mod google_play_service;
pub mod stripe_client;
pub mod stripe_error;
pub mod stripe_service;

use async_trait::async_trait;
use google_androidpublisher3::api::SubscriptionPurchase;
use lb_rs::model::api::UpgradeAccountAppStoreError;

use crate::ServerError;
use crate::config::{AppleConfig, Config};

use self::app_store_client::AppStoreClient;
use self::app_store_model::{LastTransactionItem, TransactionInfo};
use self::google_play_client::{GooglePlayClient, SimpleGCPError};
use self::stripe_client::StripeClient;
use self::stripe_error::SimplifiedStripeError;

#[derive(Clone)]
pub struct Nop {}

#[async_trait]
impl StripeClient for Nop {
    async fn create_customer(
        &self, _customer_name: String,
    ) -> Result<stripe::Customer, SimplifiedStripeError> {
        todo!()
    }

    async fn create_checkout_session(
        &self, _customer_id: stripe::CustomerId, _price_id: &str, _success_url: &str,
        _cancel_url: &str,
    ) -> Result<stripe::CheckoutSession, SimplifiedStripeError> {
        todo!()
    }

    async fn create_billing_portal_session(
        &self, _customer_id: stripe::CustomerId, _return_url: &str,
    ) -> Result<stripe::BillingPortalSession, SimplifiedStripeError> {
        todo!()
    }

    async fn cancel_subscription_at_period_end(
        &self, _subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        todo!()
    }

    async fn get_subscription(
        &self, _subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        todo!()
    }

    async fn list_subscriptions(
        &self, _customer_id: stripe::CustomerId,
    ) -> Result<Vec<stripe::Subscription>, SimplifiedStripeError> {
        todo!()
    }

    async fn expire_open_checkout_sessions(
        &self, _customer_id: stripe::CustomerId,
    ) -> Result<(), SimplifiedStripeError> {
        todo!()
    }
}

#[async_trait]
impl GooglePlayClient for Nop {
    async fn acknowledge_subscription(
        &self, _config: &Config, _purchase_token: &str,
    ) -> Result<(), SimpleGCPError> {
        todo!()
    }

    async fn cancel_subscription(
        &self, _config: &Config, _purchase_token: &str,
    ) -> Result<(), SimpleGCPError> {
        todo!()
    }

    async fn get_subscription(
        &self, _config: &Config, _purchase_token: &str,
    ) -> Result<SubscriptionPurchase, SimpleGCPError> {
        todo!()
    }
}

#[async_trait]
impl AppStoreClient for Nop {
    async fn get_sub_status(
        &self, _config: &AppleConfig, _original_transaction_id: &str,
    ) -> Result<(LastTransactionItem, TransactionInfo), ServerError<UpgradeAccountAppStoreError>>
    {
        todo!()
    }
}
