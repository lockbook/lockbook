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
pub mod stripe_model;
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
        &self, _customer_name: String, _payment_method_id: stripe::PaymentMethodId,
    ) -> Result<stripe::Customer, SimplifiedStripeError> {
        todo!()
    }

    async fn create_payment_method(
        &self, _card_number: &str, _exp_month: i32, _exp_year: i32, _cvc: &str,
    ) -> Result<stripe::PaymentMethod, SimplifiedStripeError> {
        todo!()
    }

    async fn create_setup_intent(
        &self, _customer_id: stripe::CustomerId, _payment_method_id: stripe::PaymentMethodId,
    ) -> Result<stripe::SetupIntent, SimplifiedStripeError> {
        todo!()
    }

    async fn create_subscription(
        &self, _customer_id: stripe::CustomerId, _payment_method_id: &str, _price_id: &str,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        todo!()
    }

    async fn detach_payment_method_from_customer(
        &self, _payment_method_id: &stripe::PaymentMethodId,
    ) -> Result<(), SimplifiedStripeError> {
        todo!()
    }

    async fn cancel_subscription(
        &self, _subscription_id: &stripe::SubscriptionId,
    ) -> Result<(), SimplifiedStripeError> {
        todo!()
    }

    async fn get_subscription(
        &self, _subscription_id: &stripe::SubscriptionId,
    ) -> Result<stripe::Subscription, SimplifiedStripeError> {
        todo!()
    }

    async fn retrieve_invoice(
        &self, _invoice_id: &stripe::InvoiceId,
    ) -> Result<stripe::Invoice, SimplifiedStripeError> {
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
