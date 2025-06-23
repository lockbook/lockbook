impl Lb {
    pub async fn calculate_work(&self) -> LbResult<SyncStatus>{
        match self {
            Lb::Direct(inner) => {
                inner.calculate_work().await
            }
            Lb::Network(proxy) => {
                proxy.calculate_work().await
            }
        }
    }
    pub async fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus>{
        match self {
            Lb::Direct(inner) => {
                inner.sync(f).await
            }
            Lb::Network(proxy) => {
                proxy.sync(f).await
            }
        }
    }
    pub async fn get_last_synced_human(&self) -> LbResult<String>{
        match self {
            Lb::Direct(inner) => {
                inner.get_last_synced_human().await
            }
            Lb::Network(proxy) => {
                proxy.get_last_synced_human().await
            }
        }
    }
    pub async fn get_timestamp_human_string(&self, timestamp: i64) -> String{
        match self {
            Lb::Direct(inner) => {
                inner.get_timestamp_human_string(timestamp)
            }
            Lb::Network(proxy) => {
                proxy.get_timestamp_human_string(timestamp).await
            }
        }
    }
}

use crate::{service::sync::{SyncProgress, SyncStatus}, Lb, LbResult};