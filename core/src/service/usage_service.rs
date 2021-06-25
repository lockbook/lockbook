use crate::client;
use crate::model::state::Config;
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::file_service;
use crate::CoreError;
use lockbook_models::api::{FileUsage, GetUsageRequest, GetUsageResponse};
use lockbook_models::file_metadata::FileType::Document;
use serde::Serialize;
use std::fmt;
use uuid::Uuid;

pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1000;
pub const MEGABYTE: u64 = KILOBYTE * 1000;
pub const GIGABYTE: u64 = MEGABYTE * 1000;
pub const TERABYTE: u64 = GIGABYTE * 1000;

pub const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
pub const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
pub const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
pub const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

#[derive(Serialize)]
pub enum ByteUnit {
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
    Terabyte,
}

impl fmt::Display for ByteUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let abbr = match self {
            ByteUnit::Byte => "B",
            ByteUnit::Kilobyte => "KB",
            ByteUnit::Megabyte => "MB",
            ByteUnit::Gigabyte => "GB",
            ByteUnit::Terabyte => "TB",
        };

        write!(f, "{}", abbr)
    }
}

#[derive(Serialize)]
pub struct UsageMetrics {
    pub usages: Vec<FileUsage>,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

#[derive(Serialize)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable_exact: String,
    pub readable: String,
    pub unit: ByteUnit,
}

pub fn bytes_to_human(size: u64) -> (String, ByteUnit) {
    let (unit, unit_size) = match size {
        0..=KILOBYTE => (ByteUnit::Byte, BYTE),
        KILOBYTE_PLUS_ONE..=MEGABYTE => (ByteUnit::Kilobyte, KILOBYTE),
        MEGABYTE_PLUS_ONE..=GIGABYTE => (ByteUnit::Megabyte, MEGABYTE),
        GIGABYTE_PLUS_ONE..=TERABYTE => (ByteUnit::Gigabyte, GIGABYTE),
        TERABYTE_PLUS_ONE..=u64::MAX => (ByteUnit::Terabyte, TERABYTE),
    };

    (
        format!("{:.3} {}", size as f64 / unit_size as f64, unit),
        unit,
    )
}

pub fn server_usage(config: &Config) -> Result<GetUsageResponse, CoreError> {
    let acc = account_repo::get_account(config)?;

    client::request(&acc, GetUsageRequest {}).map_err(CoreError::from)
}

pub fn get_usage(config: &Config) -> Result<UsageMetrics, CoreError> {
    let server_usage_and_cap = server_usage(&config)?;

    let server_usage = server_usage_and_cap.sum_server_usage();
    let cap = server_usage_and_cap.cap;

    let (readable_usage, usage_unit) = bytes_to_human(server_usage);
    let (readable_cap, cap_unit) = bytes_to_human(cap);

    Ok(UsageMetrics {
        usages: server_usage_and_cap.usages,
        server_usage: UsageItemMetric {
            exact: server_usage,
            readable_exact: format!("{} B", server_usage),
            readable: readable_usage,
            unit: usage_unit,
        },
        data_cap: UsageItemMetric {
            exact: cap,
            readable_exact: format!("{} B", cap),
            readable: readable_cap,
            unit: cap_unit,
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

    let (readable, unit) = bytes_to_human(local_usage);

    Ok(UsageItemMetric {
        exact: local_usage,
        readable_exact: format!("{} B", local_usage),
        readable,
        unit,
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
        assert_eq!(
            bytes_to_human(bytes_small_total),
            (format!("{}.000 KB", 2), ByteUnit::Kilobyte)
        );

        let bytes_medium_total = BYTES_MEDIUM * 2;
        assert_eq!(
            bytes_to_human(bytes_medium_total),
            (format!("{}.000 MB", 2), ByteUnit::Megabyte)
        );

        let bytes_large_total = BYTES_LARGE * 2;
        assert_eq!(
            bytes_to_human(bytes_large_total),
            (format!("{}.000 GB", 2), ByteUnit::Gigabyte)
        );
    }
}
