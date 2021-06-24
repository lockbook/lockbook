use std::convert::TryInto;

use crate::client;
use crate::model::state::Config;
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::file_service;
use crate::CoreError;
use lockbook_models::api::{GetUsageRequest, GetUsageResponse};
use lockbook_models::file_metadata::FileType::Document;
use serde::Serialize;
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
pub struct LocalAndServerUsages {
    pub metrics: UsageMetrics,
    pub readable_metrics: ReadableUsageMetrics,
}

#[derive(Serialize)]
pub struct ReadableUsageMetrics {
    pub server_usage: String,
    pub uncompressed_usage: String,
    pub data_cap: String,
}

#[derive(Serialize)]
pub struct UsageMetrics {
    pub server_usage: u64,
    pub uncompressed_usage: u64,
    pub data_cap: u64,
}

pub fn bytes_to_human(size: u64) -> String {
    let (unit, abbr) = match size {
        0..=KILOBYTE => (BYTE, ""),
        KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
        MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
        GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
        TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
    };

    format!("{:.3} {}B", size as f64 / unit as f64, abbr)
}

pub fn server_usage(config: &Config) -> Result<GetUsageResponse, CoreError> {
    let acc = account_repo::get_account(config)?;

    client::request(&acc, GetUsageRequest {}).map_err(CoreError::from)
}

pub fn get_usage_human_string(config: &Config, exact: bool) -> Result<String, CoreError> {
    let usage = server_usage(config)?.sum_server_usage();

    if exact {
        Ok(format!("{} B", usage))
    } else {
        Ok(bytes_to_human(usage))
    }
}

pub fn get_uncompressed_usage(config: &Config) -> Result<usize, CoreError> {
    let doc_ids: Vec<Uuid> = file_metadata_repo::get_all(&config)?
        .into_iter()
        .filter(|f| f.file_type == Document)
        .map(|f| f.id)
        .collect();

    let mut size: usize = 0;
    for id in doc_ids {
        size += file_service::read_document(&config, id)?.len()
    }

    Ok(size)
}

pub fn local_and_server_usages(
    config: &Config,
    exact: bool,
) -> Result<LocalAndServerUsages, CoreError> {
    let server_usage_and_cap = server_usage(&config)?;

    let server_usage = server_usage_and_cap.sum_server_usage();
    let local_usage = get_uncompressed_usage(config)?;
    let cap = server_usage_and_cap.cap;

    let metrics = UsageMetrics {
        server_usage,
        uncompressed_usage: local_usage as u64,
        data_cap: cap,
    };

    let readable_metrics =
        if exact {
            ReadableUsageMetrics {
                server_usage: format!("{} B", server_usage),
                uncompressed_usage: format!("{} bytes", local_usage),
                data_cap: format!("{} B", cap),
            }
        } else {
            ReadableUsageMetrics {
                server_usage: bytes_to_human(server_usage),
                uncompressed_usage: bytes_to_human(local_usage.try_into().map_err(|_| {
                    CoreError::Unexpected(String::from("uncompressed size too large"))
                })?),
                data_cap: bytes_to_human(cap),
            }
        };

    Ok(LocalAndServerUsages {
        metrics,
        readable_metrics,
    })
}

#[cfg(test)]
mod unit_tests {
    use crate::service::usage_service::bytes_to_human;

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
