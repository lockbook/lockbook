use crate::client::Client;
use crate::model::api;
use crate::model::api::{GetUsageRequest, GetUsageResponse};
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::storage::db_provider::Backend;
use crate::{client, DefaultClient};

pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1000;
pub const MEGABYTE: u64 = KILOBYTE * 1000;
pub const GIGABYTE: u64 = MEGABYTE * 1000;
pub const TERABYTE: u64 = GIGABYTE * 1000;

pub const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
pub const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
pub const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
pub const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

pub enum GetUsageError<MyBackend: Backend> {
    AccountRetrievalError(account_repo::AccountRepoError<MyBackend>),
    ApiError(client::ApiError<api::GetUsageError>),
}

pub trait UsageService<MyBackend: Backend, AccountDb: AccountRepo<MyBackend>> {
    fn get_usage(backend: &MyBackend::Db) -> Result<GetUsageResponse, GetUsageError<MyBackend>>;
    fn get_usage_human_string(
        backend: &MyBackend::Db,
        exact: bool,
    ) -> Result<String, GetUsageError<MyBackend>>;
}

pub struct UsageServiceImpl<MyBackend: Backend, AccountDb: AccountRepo<MyBackend>> {
    _accounts: AccountDb,
    _backend: MyBackend,
}

impl<MyBackend: Backend, AccountDb: AccountRepo<MyBackend>> UsageService<MyBackend, AccountDb>
    for UsageServiceImpl<MyBackend, AccountDb>
{
    fn get_usage(backend: &MyBackend::Db) -> Result<GetUsageResponse, GetUsageError<MyBackend>> {
        let acc = AccountDb::get_account(backend).map_err(GetUsageError::AccountRetrievalError)?;

        DefaultClient::request(&acc, GetUsageRequest {}).map_err(GetUsageError::ApiError)
    }

    fn get_usage_human_string(
        backend: &MyBackend::Db,
        exact: bool,
    ) -> Result<String, GetUsageError<MyBackend>> {
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
