use std::collections::HashMap;

use lockbook_shared::document_repo::DocumentService;
use serde::Serialize;

use lockbook_shared::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::usage::bytes_to_human;
use uuid::Uuid;

use crate::{CoreError, Requester};
use crate::{CoreState, LbResult};

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

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    fn server_usage(&self) -> LbResult<GetUsageResponse> {
        let acc = &self.get_account()?;

        Ok(self.client.request(acc, GetUsageRequest {})?)
    }

    pub(crate) fn get_usage(&self) -> LbResult<UsageMetrics> {
        let server_usage_and_cap = self.server_usage()?;

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

    pub(crate) fn get_uncompressed_usage_breakdown(&mut self) -> LbResult<HashMap<Uuid, usize>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;
        let mut breakdown = HashMap::default();

        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = tree.read_document(&self.docs, &id, account)?;
                let doc_size = doc.len();
                breakdown.insert(id, doc_size);
            }
        }

        Ok(breakdown)
    }

    pub(crate) fn get_uncompressed_usage(&mut self) -> LbResult<UsageItemMetric> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;

        let mut local_usage: u64 = 0;
        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let doc = tree.read_document(&self.docs, &id, account)?;
                local_usage += doc.len() as u64
            }
        }

        let readable = bytes_to_human(local_usage);
        Ok(UsageItemMetric { exact: local_usage, readable })
    }
}
