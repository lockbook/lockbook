use crate::logic::api::{FileUsage, GetUsageResponse};
use crate::logic::file_like::FileLike;
use crate::logic::tree_like::TreeLike;
use crate::logic::usage::bytes_to_human;
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
    pub fn get_usage(&self, server_usage_and_cap: GetUsageResponse) -> LbResult<UsageMetrics> {
        let server_usage = server_usage_and_cap.sum_server_usage();
        let cap = server_usage_and_cap.cap;

        let readable_usage = bytes_to_human(server_usage);
        let readable_cap = bytes_to_human(cap);

        Ok(UsageMetrics {
            usages: server_usage_and_cap.usages,
            server_usage: UsageItemMetric { exact: server_usage, readable: readable_usage },
            data_cap: UsageItemMetric { exact: cap, readable: readable_cap },
        })
    }

    pub async fn get_uncompressed_usage_breakdown(&mut self) -> LbResult<HashMap<Uuid, usize>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;
        let mut breakdown = HashMap::default();

        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = tree.read_document(&self.docs, &id, account).await?;
                let doc_size = doc.len();
                breakdown.insert(id, doc_size);
            }
        }

        Ok(breakdown)
    }

    // big async opportunity
    pub async fn get_uncompressed_usage(&mut self) -> LbResult<UsageItemMetric> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let mut local_usage: u64 = 0;
        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = tree.read_document(&self.docs, &id, account).await?;
                local_usage += doc.len() as u64
            }
        }

        let readable = bytes_to_human(local_usage);
        Ok(UsageItemMetric { exact: local_usage, readable })
    }
}
