use serde::Serialize;

use lockbook_shared::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_shared::file::like::FileLike;
use lockbook_shared::tree::lazy::LazyTreeLike;
use lockbook_shared::tree::like::TreeLike;
use lockbook_shared::tree::stagable::Stagable;
use lockbook_shared::usage::bytes_to_human;

use crate::{CoreError, RequestContext, Requester};
use crate::{CoreResult, OneKey};

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

impl<Client: Requester> RequestContext<'_, '_, Client> {
    fn server_usage(&self) -> CoreResult<GetUsageResponse> {
        let acc = &self.get_account()?;

        Ok(self.client.request(acc, GetUsageRequest {})?)
    }

    pub fn get_usage(&self) -> CoreResult<UsageMetrics> {
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

    pub fn get_uncompressed_usage(&mut self) -> CoreResult<UsageItemMetric> {
        let mut tree = (&mut self.tx.base_metadata)
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut local_usage: u64 = 0;
        for id in tree.owned_ids() {
            let is_file_deleted = tree.calculate_deleted(&id)?;
            let file = tree.find(&id)?;

            if !is_file_deleted && file.is_document() {
                let result = tree.read_document(self.config, &id, account)?;
                tree = result.0;
                let doc = result.1;

                local_usage += doc.len() as u64
            }
        }

        let readable = bytes_to_human(local_usage);
        Ok(UsageItemMetric { exact: local_usage, readable })
    }
}
