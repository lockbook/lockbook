use crate::{FREE_TIER_USAGE_SIZE, PREMIUM_TIER_USAGE_SIZE};
use lockbook_models::api::{GooglePlayAccountState, UnixTimeMillis};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SubscriptionProfile {
    pub billing_platform: Option<BillingPlatform>,
    pub last_in_payment_flow: u64,
}

impl SubscriptionProfile {
    pub fn data_cap(&self) -> u64 {
        match &self.billing_platform {
            Some(platform) => match platform {
                BillingPlatform::Stripe(_) => PREMIUM_TIER_USAGE_SIZE,
                BillingPlatform::GooglePlay(info) => {
                    if info.account_state == GooglePlayAccountState::OnHold {
                        FREE_TIER_USAGE_SIZE
                    } else {
                        PREMIUM_TIER_USAGE_SIZE
                    }
                }
            },
            None => FREE_TIER_USAGE_SIZE,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BillingPlatform {
    Stripe(StripeUserInfo),
    GooglePlay(GooglePlayUserInfo),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GooglePlayUserInfo {
    pub purchase_token: String,
    pub subscription_product_id: String,
    pub subscription_offer_id: String,
    pub expiration_time: UnixTimeMillis,
    pub account_state: GooglePlayAccountState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StripeUserInfo {
    pub customer_id: String,
    pub customer_name: Uuid,
    pub price_id: String,
    pub payment_method_id: String,
    pub last_4: String,
    pub subscription_id: String,
    pub expiration_time: UnixTimeMillis,
}
