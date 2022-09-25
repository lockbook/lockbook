use serde::Serialize;

use lockbook_shared::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::tree_like::{Stagable, TreeLike};

use crate::{CoreError, RequestContext, Requester};
use crate::{CoreResult, OneKey};

const BYTE: u64 = 1;
const KILOBYTE: u64 = BYTE * 1000;
const MEGABYTE: u64 = KILOBYTE * 1000;
const GIGABYTE: u64 = MEGABYTE * 1000;
const TERABYTE: u64 = GIGABYTE * 1000;

const KILOBYTE_MINUS_ONE: u64 = KILOBYTE - 1;
const MEGABYTE_MINUS_ONE: u64 = MEGABYTE - 1;
const GIGABYTE_MINUS_ONE: u64 = GIGABYTE - 1;
const TERABYTE_MINUS_ONE: u64 = TERABYTE - 1;

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
        let mut tree = self
            .tx
            .base_metadata
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

pub fn bytes_to_human(size: u64) -> String {
    let (unit, abbr) = match size {
        0..=KILOBYTE_MINUS_ONE => (BYTE, "B"),
        KILOBYTE..=MEGABYTE_MINUS_ONE => (KILOBYTE, "KB"),
        MEGABYTE..=GIGABYTE_MINUS_ONE => (MEGABYTE, "MB"),
        GIGABYTE..=TERABYTE_MINUS_ONE => (GIGABYTE, "GB"),
        TERABYTE..=u64::MAX => (TERABYTE, "TB"),
    };

    let size_in_unit = size as f64 / unit as f64;
    let dec = f64::trunc(size_in_unit.fract() * 100.0) / 100.0;

    let num = format!("{:.2}", size_in_unit.trunc() + dec)
        .trim_end_matches(['0'])
        .trim_end_matches(['.'])
        .to_owned();

    format!("{} {}", num, abbr)
}
