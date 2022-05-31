use serde::{Deserialize, Serialize};
use uuid::Uuid;
use lockbook_models::api::GooglePlayAccountState;

#[derive(Serialize, Deserialize, Debug)]
pub struct SubscriptionHistory {
    pub info: Vec<BillingInfo>,
    pub last_in_payment_flow: u64,
}

impl Default for SubscriptionHistory {
    fn default() -> Self {
        SubscriptionHistory { info: vec![], last_in_payment_flow: 0 }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BillingInfo {
    Stripe(StripeUserInfo),
    GooglePlay(GooglePlayUserInfo),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GooglePlayUserInfo {
    pub purchase_token: String,
    pub subscription_product_id: String,
    pub subscription_offer_id: String,
    pub expiration_time: u64,
    pub account_state: GooglePlayAccountState
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StripeUserInfo {
    pub customer_id: String,
    pub customer_name: Uuid,
    pub payment_method_id: String,
    pub last_4: String,
    pub subscription_id: String,
    pub expiration_time: u64,
}
