impl Lb {
    pub async fn get_usage(&self) -> LbResult<UsageMetrics>{
        match self {
            Lb::Direct(inner) => {
                inner.get_usage().await
            }
            Lb::Network(proxy) => {
                proxy.get_usage().await
            }
        }
    }
    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>>{
        match self {
            Lb::Direct(inner) => {
                inner.get_uncompressed_usage_breakdown().await
            }
            Lb::Network(proxy) => {
                proxy.get_uncompressed_usage_breakdown().await
            }
        }
    }
    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric>{
        match self {
            Lb::Direct(inner) => {
                inner.get_uncompressed_usage().await
            }
            Lb::Network(proxy) => {
                proxy.get_uncompressed_usage().await
            }
        }
    }

}

use std::collections::HashMap;
use uuid::Uuid;
use crate::{service::usage::{UsageItemMetric, UsageMetrics}, Lb, LbResult};