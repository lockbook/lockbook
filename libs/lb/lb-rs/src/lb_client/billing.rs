impl LbClient {
    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&account_tier)
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "upgrade_account_stripe", Some(args)).await
    }
    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(purchase_token.to_string(),account_id.to_string()))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "upgrade_account_google_play", Some(args)).await
    }
    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(original_transaction_id,app_account_token))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "upgrade_account_app_store", Some(args)).await
    }
    pub async fn cancel_subscription(&self) -> LbResult<()>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "cancel_subscription", None).await
    }
    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>>{
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "get_subscription_info", None).await
    }
}

use crate::lb_client::LbClient;
use crate::model::api::{StripeAccountTier, SubscriptionInfo};
use crate::{model::errors::core_err_unexpected, LbResult};
use tokio::net::TcpStream;
use crate::rpc::call_rpc;