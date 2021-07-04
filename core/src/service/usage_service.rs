use crate::client;
use crate::model::state::Config;
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::file_service;
use crate::CoreError;
use lockbook_models::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_models::file_metadata::FileType::Document;
use serde::Serialize;
use uuid::Uuid;

pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1000;
pub const MEGABYTE: u64 = KILOBYTE * 1000;
pub const GIGABYTE: u64 = MEGABYTE * 1000;
pub const TERABYTE: u64 = GIGABYTE * 1000;

pub const KILOBYTE_MINUS_ONE: u64 = KILOBYTE - 1;
pub const MEGABYTE_MINUS_ONE: u64 = MEGABYTE - 1;
pub const GIGABYTE_MINUS_ONE: u64 = GIGABYTE - 1;
pub const TERABYTE_MINUS_ONE: u64 = TERABYTE - 1;

#[derive(Serialize)]
pub struct UsageMetrics {
    pub usages: Vec<FileUsage>,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

#[derive(Serialize)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable: String,
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

pub fn server_usage(config: &Config) -> Result<GetUsageResponse, CoreError> {
    let acc = account_repo::get_account(config)?;

    client::request(&acc, GetUsageRequest {}).map_err(CoreError::from)
}

pub fn get_usage(config: &Config) -> Result<UsageMetrics, CoreError> {
    let server_usage_and_cap = server_usage(&config)?;

    let server_usage = server_usage_and_cap.sum_server_usage();
    let cap = server_usage_and_cap.cap;

    let readable_usage = bytes_to_human(server_usage);
    let readable_cap = bytes_to_human(cap);

    Ok(UsageMetrics {
        usages: server_usage_and_cap.usages,
        server_usage: UsageItemMetric {
            exact: server_usage,
            readable: readable_usage,
        },
        data_cap: UsageItemMetric {
            exact: cap,
            readable: readable_cap,
        },
    })
}

pub fn get_uncompressed_usage(config: &Config) -> Result<UsageItemMetric, CoreError> {
    let doc_ids: Vec<Uuid> = file_metadata_repo::get_all(&config)?
        .into_iter()
        .filter(|f| f.file_type == Document)
        .map(|f| f.id)
        .collect();

    let mut local_usage: u64 = 0;
    for id in doc_ids {
        local_usage += file_service::read_document(&config, id)?.len() as u64
    }

    let readable = bytes_to_human(local_usage);

    Ok(UsageItemMetric {
        exact: local_usage,
        readable,
    })
}

#[cfg(test)]
mod unit_tests {
    use crate::service::usage_service::{bytes_to_human, ByteUnit};

    const BYTES_SMALL: u64 = 1000;
    const BYTES_MEDIUM: u64 = 1000000;
    const BYTES_LARGE: u64 = 1000000000;

    #[test]
    fn usage_human_string_sanity_check() {
        let bytes_small_total = BYTES_SMALL * 2;
        assert_eq!(bytes_to_human(bytes_small_total), format!("{}.000 KB", 2));

        let bytes_medium_total = BYTES_MEDIUM * 2;
        assert_eq!(bytes_to_human(bytes_medium_total), format!("{}.000 MB", 2));

        let bytes_large_total = BYTES_LARGE * 2;
        assert_eq!(bytes_to_human(bytes_large_total), format!("{}.000 GB", 2));
    }
}
