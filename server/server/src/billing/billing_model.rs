use crate::FREE_TIER_USAGE_SIZE;
use lockbook_models::api::GooglePlayAccountState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct SubscriptionProfile {
    pub info: Option<BillingPlatform>,
    pub last_in_payment_flow: u64,
    pub data_cap: u64,
}

impl Default for SubscriptionProfile {
    fn default() -> Self {
        SubscriptionProfile { info: None, last_in_payment_flow: 0, data_cap: FREE_TIER_USAGE_SIZE }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BillingPlatform {
    Stripe(StripeUserInfo),
    GooglePlay(GooglePlayUserInfo),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GooglePlayUserInfo {
    pub purchase_token: String,
    pub subscription_product_id: String,
    pub subscription_offer_id: String,
    pub expiration_time: u64,
    pub account_state: GooglePlayAccountState,
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
