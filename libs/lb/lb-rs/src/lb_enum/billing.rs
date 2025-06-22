impl Lb {
    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.upgrade_account_stripe(account_tier).await
            }
            Lb::Network(proxy) => {
                proxy.upgrade_account_stripe(account_tier).await
            }
        }
    }
    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.upgrade_account_google_play(purchase_token, account_id).await
            }
            Lb::Network(proxy) => {
                proxy.upgrade_account_google_play(purchase_token, account_id).await
            }
        }
    }
    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.upgrade_account_app_store(original_transaction_id,app_account_token).await
            }
            Lb::Network(proxy) => {
                proxy.upgrade_account_app_store(original_transaction_id,app_account_token).await
            }
        }
    }
    pub async fn cancel_subscription(&self) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.cancel_subscription().await
            }
            Lb::Network(proxy) => {
                proxy.cancel_subscription().await
            }
        }
    }
    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>>{
        match self {
            Lb::Direct(inner) => {
                inner.get_subscription_info().await
            }
            Lb::Network(proxy) => {
                proxy.get_subscription_info().await
            }
        }
    }
}

use crate::model::api::{StripeAccountTier, SubscriptionInfo};
use crate::{Lb, LbResult};