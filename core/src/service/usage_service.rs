use crate::client;
use crate::client::Client;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo::{DbError, FileMetadataRepo};
use crate::service::file_service::{FileService, ReadDocumentError};
use lockbook_models::api;
use lockbook_models::api::{GetUsageRequest, GetUsageResponse};
use lockbook_models::file_metadata::FileType::Document;

use crate::model::state::Config;
use std::convert::TryInto;
use std::num::TryFromIntError;
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

#[derive(Debug)]
pub enum GetUsageError {
    AccountRetrievalError(account_repo::AccountRepoError),
    ApiError(client::ApiError<api::GetUsageError>),
}

#[derive(Debug)]
pub enum UncompressedError {
    FileMetadataDb(DbError),
    FilesError(ReadDocumentError),
}

#[derive(Debug)]
pub enum LocalAndServerUsageError {
    GetUsageError(GetUsageError),
    CalcUncompressedError(UncompressedError),
    UncompressedNumberTooLarge(TryFromIntError),
}

pub struct LocalAndServerUsages {
    pub server_usage: String,
    pub uncomressed_usage: String,
    pub data_cap: String,
}

pub trait UsageService {
    fn bytes_to_human(size: u64) -> String;
    fn server_usage(config: &Config) -> Result<GetUsageResponse, GetUsageError>;
    fn get_usage_human_string(config: &Config, exact: bool) -> Result<String, GetUsageError>;
    fn get_uncompressed_usage(config: &Config) -> Result<usize, UncompressedError>;
    fn local_and_server_usages(
        config: &Config,
        exact: bool,
    ) -> Result<LocalAndServerUsages, LocalAndServerUsageError>;
}

pub struct UsageServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    Files: FileService,
    AccountDb: AccountRepo,
    ApiClient: Client,
> {
    _accounts: AccountDb,

    _client: ApiClient,
    _files: Files,
    _files_db: FileMetadataDb,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        Files: FileService,
        AccountDb: AccountRepo,
        ApiClient: Client,
    > UsageService for UsageServiceImpl<FileMetadataDb, Files, AccountDb, ApiClient>
{
    fn bytes_to_human(size: u64) -> String {
        let (unit, abbr) = match size {
            0..=KILOBYTE => (BYTE, ""),
            KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
            MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
            GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
            TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
        };

        format!("{:.3} {}B", size as f64 / unit as f64, abbr)
    }

    fn server_usage(config: &Config) -> Result<GetUsageResponse, GetUsageError> {
        let acc = AccountDb::get_account(config).map_err(GetUsageError::AccountRetrievalError)?;

        ApiClient::request(&acc, GetUsageRequest {}).map_err(GetUsageError::ApiError)
    }

    fn get_usage_human_string(config: &Config, exact: bool) -> Result<String, GetUsageError> {
        let usage = Self::server_usage(config)?.sum_server_usage();

        if exact {
            Ok(format!("{} B", usage))
        } else {
            Ok(Self::bytes_to_human(usage))
        }
    }

    fn get_uncompressed_usage(config: &Config) -> Result<usize, UncompressedError> {
        let doc_ids: Vec<Uuid> = FileMetadataDb::get_all(&config)
            .map_err(UncompressedError::FileMetadataDb)?
            .into_iter()
            .filter(|f| f.file_type == Document)
            .map(|f| f.id)
            .collect();

        let mut size: usize = 0;
        for id in doc_ids {
            size += Files::read_document(&config, id)
                .map_err(UncompressedError::FilesError)?
                .len()
        }

        Ok(size)
    }

    fn local_and_server_usages(
        config: &Config,
        exact: bool,
    ) -> Result<LocalAndServerUsages, LocalAndServerUsageError> {
        let server_usage_and_cap =
            Self::server_usage(&config).map_err(LocalAndServerUsageError::GetUsageError)?;

        let server_usage = server_usage_and_cap.sum_server_usage();
        let local_usage = Self::get_uncompressed_usage(config)
            .map_err(LocalAndServerUsageError::CalcUncompressedError)?;
        let cap = server_usage_and_cap.cap;

        let usages = if exact {
            LocalAndServerUsages {
                server_usage: format!("{} B", server_usage),
                uncomressed_usage: format!("{} bytes", local_usage),
                data_cap: format!("{} B", cap),
            }
        } else {
            LocalAndServerUsages {
                server_usage: Self::bytes_to_human(server_usage),
                uncomressed_usage: Self::bytes_to_human(
                    local_usage
                        .try_into()
                        .map_err(LocalAndServerUsageError::UncompressedNumberTooLarge)?,
                ),
                data_cap: Self::bytes_to_human(cap),
            }
        };

        Ok(usages)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::{ApiError, Client};
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoError};
    use crate::service::usage_service::{UsageService, UsageServiceImpl};
    use crate::{DefaultFileMetadataRepo, DefaultFileService, DefaultPKCrypto};
    
    use lockbook_crypto::pubkey::PubKeyCryptoService;
    use lockbook_models::account::{Account, ApiUrl};
    use lockbook_models::api::{FileUsage, GetUsageResponse, Request};
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use uuid::Uuid;

    const BYTES_SMALL: u64 = 1000;
    const BYTES_MEDIUM: u64 = 1000000;
    const BYTES_LARGE: u64 = 1000000000;

    struct MockAccountRepo {}

    impl AccountRepo for MockAccountRepo {
        fn insert_account(_config: &Config, _account: &Account) -> Result<(), AccountRepoError> {
            unimplemented!()
        }

        fn maybe_get_account(_config: &Config) -> Result<Option<Account>, AccountRepoError> {
            unimplemented!()
        }

        fn get_account(_config: &Config) -> Result<Account, AccountRepoError> {
            Ok(Account {
                username: "".to_string(),
                api_url: "".to_string(),
                private_key: DefaultPKCrypto::generate_key(),
            })
        }

        fn get_api_url(_config: &Config) -> Result<ApiUrl, AccountRepoError> {
            unimplemented!()
        }
    }

    trait BytesHelper {
        fn get_bytes_count() -> u64;
    }

    struct BytesHelperSmall;

    impl BytesHelper for BytesHelperSmall {
        fn get_bytes_count() -> u64 {
            BYTES_SMALL
        }
    }

    struct BytesHelperMedium;

    impl BytesHelper for BytesHelperMedium {
        fn get_bytes_count() -> u64 {
            BYTES_MEDIUM
        }
    }

    struct BytesHelperLarge;

    impl BytesHelper for BytesHelperLarge {
        fn get_bytes_count() -> u64 {
            BYTES_LARGE
        }
    }

    struct MockClient<MyByteHelper: BytesHelper> {
        _byte_helper: MyByteHelper,
    }

    impl<MyByteHelper: BytesHelper> Client for MockClient<MyByteHelper> {
        fn request<
            T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
        >(
            _account: &Account,
            _request: T,
        ) -> Result<<T as Request>::Response, ApiError<<T as Request>::Error>> {
            let file_usage = FileUsage {
                file_id: Uuid::nil(),
                size_bytes: MyByteHelper::get_bytes_count(),
            };

            let get_usage_response = GetUsageResponse {
                usages: vec![file_usage.clone(), file_usage],
                cap: 100000,
            };

            let serialized = serde_json::to_string(&get_usage_response).unwrap();
            let deserialized: T::Response = serde_json::from_str(&serialized).unwrap();

            Ok(deserialized)
        }
    }

    fn get_usage_of_size<T: BytesHelper>(exact: bool) -> String {
        UsageServiceImpl::<
            DefaultFileMetadataRepo,
            DefaultFileService,
            MockAccountRepo,
            MockClient<T>,
        >::get_usage_human_string(
            &Config {
                writeable_path: "".to_string(),
            },
            exact,
        )
        .unwrap()
    }

    #[test]
    fn usage_human_string_sanity_check() {
        let bytes_small_result = get_usage_of_size::<BytesHelperSmall>(false);
        let bytes_small_exact_result = get_usage_of_size::<BytesHelperSmall>(true);
        let bytes_small_total = BYTES_SMALL * 2;

        assert_eq!(bytes_small_result, format!("{}.000 KB", 2));
        assert_eq!(bytes_small_exact_result, format!("{} B", bytes_small_total));

        let bytes_medium_result = get_usage_of_size::<BytesHelperMedium>(false);
        let bytes_medium_exact_result = get_usage_of_size::<BytesHelperMedium>(true);
        let bytes_medium_total = BYTES_MEDIUM * 2;

        assert_eq!(bytes_medium_result, format!("{}.000 MB", 2));
        assert_eq!(
            bytes_medium_exact_result,
            format!("{} B", bytes_medium_total)
        );

        let bytes_large_result = get_usage_of_size::<BytesHelperLarge>(false);
        let bytes_large_exact_result = get_usage_of_size::<BytesHelperLarge>(true);
        let bytes_large_total = BYTES_LARGE * 2;

        assert_eq!(bytes_large_result, format!("{}.000 GB", 2));
        assert_eq!(bytes_large_exact_result, format!("{} B", bytes_large_total));
    }
}
