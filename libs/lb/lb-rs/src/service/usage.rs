use crate::Lb;
use crate::model::api::{FileUsage, GetUsageRequest};
use crate::model::errors::LbResult;
use crate::model::usage::get_usage;
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct UsageMetrics {
    pub usages: Vec<FileUsage>,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

#[derive(Serialize, PartialEq, Eq, Debug, Clone)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable: String,
}

impl Lb {
    /// fetches data footprint on server along with data cap information
    /// compares this to local changes to estimate net data increase
    ///
    /// callers of this function should be prepared to handle:
    /// - [crate::LbErrKind::AccountNonexistent]
    /// - [crate::LbErrKind::ClientUpdateRequired]
    /// - [crate::LbErrKind::ServerUnreachable]
    #[instrument(level = "debug", skip(self))]
    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        let acc = self.get_account()?;
        let usage = self.client.request(acc, GetUsageRequest {}).await?;
        Ok(get_usage(usage))
    }
}
