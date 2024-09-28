use crate::logic::file_like::FileLike;
use crate::logic::tree_like::TreeLike;
use crate::logic::usage::{bytes_to_human, get_usage};
use crate::model::api::{FileUsage, GetUsageRequest};
use crate::model::errors::LbResult;
use crate::Lb;
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Debug)]
pub struct UsageMetrics {
    pub usages: Vec<FileUsage>,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

#[derive(Serialize, PartialEq, Eq, Debug)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable: String,
}

impl Lb {
    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        let acc = self.get_account()?;
        let usage = self.client.request(&acc, GetUsageRequest {}).await?;
        Ok(get_usage(usage))
    }

    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let mut breakdown = HashMap::default();

        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = self.read_document_helper(id, &mut tree).await?;
                let doc_size = doc.len();
                breakdown.insert(id, doc_size);
            }
        }

        Ok(breakdown)
    }

    // big async opportunity
    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut local_usage: u64 = 0;
        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = self.read_document_helper(id, &mut tree).await?;
                local_usage += doc.len() as u64
            }
        }

        let readable = bytes_to_human(local_usage);
        Ok(UsageItemMetric { exact: local_usage, readable })
    }
}
