use crate::client::Client;
use crate::model::api;
use crate::model::api::{GetUsageRequest, GetUsageResponse};
use crate::repo::account_repo::AccountRepo;
use crate::storage::db_provider::Backend;
use crate::{client, DefaultAccountRepo, DefaultClient};

pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1000;
pub const MEGABYTE: u64 = KILOBYTE * 1000;
pub const GIGABYTE: u64 = MEGABYTE * 1000;
pub const TERABYTE: u64 = GIGABYTE * 1000;

pub const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
pub const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
pub const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
pub const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

pub enum GetUsageError {
    NoAccount,
    ApiError(client::ApiError<api::GetUsageError>),
}

pub trait UsageService {
    fn get_usage(backend: &Backend) -> Result<GetUsageResponse, GetUsageError>;
    fn get_usage_human_string(backend: &Backend, exact: bool) -> Result<String, GetUsageError>;
}

pub struct UsageServiceImpl;

impl UsageService for UsageServiceImpl {
    fn get_usage(backend: &Backend) -> Result<GetUsageResponse, GetUsageError> {
        let acc = DefaultAccountRepo::get_account(backend).map_err(|_| GetUsageError::NoAccount)?;

        DefaultClient::request(&acc, GetUsageRequest {}).map_err(|err| GetUsageError::ApiError(err))
    }

    fn get_usage_human_string(backend: &Backend, exact: bool) -> Result<String, GetUsageError> {
        let usage_in_bytes: u64 = Self::get_usage(backend)?
            .usages
            .into_iter()
            .map(|usage| usage.byte_secs)
            .sum();

        if exact {
            Ok(usage_in_bytes.to_string())
        } else {
            let (unit, abbr) = match usage_in_bytes {
                0..=KILOBYTE => (BYTE, ""),
                KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
                MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
                GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
                TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
            };

            Ok(format!(
                "{:.3} {}B",
                usage_in_bytes as f64 / unit as f64,
                abbr
            ))
        }
    }
}
