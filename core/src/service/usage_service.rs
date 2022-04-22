use serde::Serialize;

use lockbook_models::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_models::tree::FileMetaExt;

use crate::model::repo::RepoSource;
use crate::service::{api_service, file_service};
use crate::{Config, CoreError, Tx};

pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1000;
pub const MEGABYTE: u64 = KILOBYTE * 1000;
pub const GIGABYTE: u64 = MEGABYTE * 1000;
pub const TERABYTE: u64 = GIGABYTE * 1000;

pub const KILOBYTE_MINUS_ONE: u64 = KILOBYTE - 1;
pub const MEGABYTE_MINUS_ONE: u64 = MEGABYTE - 1;
pub const GIGABYTE_MINUS_ONE: u64 = GIGABYTE - 1;
pub const TERABYTE_MINUS_ONE: u64 = TERABYTE - 1;

#[derive(Serialize, Debug)]
pub struct UsageMetrics {
    pub usages: Vec<FileUsage>,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

#[derive(Serialize, PartialEq, Debug)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable: String,
}

impl Tx<'_> {
    fn server_usage(&self) -> Result<GetUsageResponse, CoreError> {
        let acc = &self.get_account()?;

        Ok(api_service::request(acc, GetUsageRequest {})?)
    }

    pub fn get_usage(&self) -> Result<UsageMetrics, CoreError> {
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

    pub fn get_uncompressed_usage(&self, config: &Config) -> Result<UsageItemMetric, CoreError> {
        let files = self.get_all_metadata(RepoSource::Local)?;
        let docs = files.filter_documents();

        let mut local_usage: u64 = 0;
        for doc in docs {
            local_usage += file_service::get_document(config, RepoSource::Local, &doc)?.len() as u64
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

    format!("{} {}", size_in_unit.trunc() + dec, abbr)
}
