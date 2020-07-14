extern crate reqwest;

#[macro_use]
extern crate log;
use crate::client::{ClientImpl, Error};
use crate::model::account::Account;
use crate::model::api::NewAccountError;
use crate::model::file_metadata::FileMetadata;
use crate::model::state::Config;
use crate::repo::account_repo::{AccountRepo, AccountRepoError, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::document_repo::DocumentRepoImpl;
use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{
    AccountCreationError, AccountImportError, AccountService, AccountServiceImpl,
};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{
    FileService, FileServiceImpl, NewFileError, NewFileFromPathError,
};
use crate::service::sync_service::FileSyncService;
use crate::CreateAccountError::{CouldNotReachServer, InvalidUsername, UsernameTaken};
use crate::CreateFileAtPathEnum::{
    DocumentTreatedAsFolder, FileAlreadyExists, NoRoot, PathDoesntStartWithRoot,
};
use crate::ImportError::AccountStringCorrupted;
pub use sled::Db;

pub mod c_interface;
pub mod client;
pub mod model;
pub mod repo;
pub mod service;

mod java_interface;

static API_URL: &str = env!("API_URL");
static DB_NAME: &str = "lockbook.sled";

pub type DefaultCrypto = RsaImpl;
pub type DefaultSymmetric = AesImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultClient = ClientImpl;
pub type DefaultAccountRepo = AccountRepoImpl;
pub type DefaultClock = ClockImpl;
pub type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultLocalChangesRepo = LocalChangesRepoImpl;
pub type DefaultDocumentRepo = DocumentRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultLocalChangesRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultLocalChangesRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;

pub fn init_logger_safely() {
    env_logger::init();
    info!("envvar RUST_LOG is {:?}", std::env::var("RUST_LOG"));
}

fn connect_to_db(config: &Config) -> Result<Db, String> {
    let db = DefaultDbProvider::connect_to_db(&config).map_err(|err| {
        format!(
            "Could not connect to db, config: {:#?}, error: {:#?}",
            &config, err
        )
    })?;

    Ok(db)
}

pub enum CreateAccountError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    UnexpectedError(String),
}

pub fn create_account(config: &Config, username: &str) -> Result<(), CreateAccountError> {
    let db = connect_to_db(&config).map_err(CreateAccountError::UnexpectedError)?;

    match DefaultAccountService::create_account(&db, username) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountCreationError::ApiError(network) => match network {
                Error::Api(api_err) => match api_err {
                    NewAccountError::UsernameTaken => Err(UsernameTaken),
                    NewAccountError::InvalidUsername => Err(InvalidUsername),
                    NewAccountError::InternalError
                    | NewAccountError::InvalidAuth
                    | NewAccountError::ExpiredAuth
                    | NewAccountError::InvalidPublicKey
                    | NewAccountError::InvalidUserAccessKey
                    | NewAccountError::FileIdTaken => Err(CreateAccountError::UnexpectedError(
                        format!("{:#?}", api_err),
                    )),
                },
                Error::SendFailed(_) => Err(CouldNotReachServer),
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => Err(
                    CreateAccountError::UnexpectedError(format!("{:#?}", network)),
                ),
            },
            AccountCreationError::KeyGenerationError(_)
            | AccountCreationError::PersistenceError(_)
            | AccountCreationError::FolderError(_)
            | AccountCreationError::MetadataRepoError(_)
            | AccountCreationError::KeySerializationError(_)
            | AccountCreationError::AuthGenFailure(_) => {
                Err(CreateAccountError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

pub enum ImportError {
    AccountStringCorrupted,
    UnexpectedError(String),
}

pub fn import_account(config: &Config, account_string: &str) -> Result<(), ImportError> {
    let db = connect_to_db(&config).map_err(ImportError::UnexpectedError)?;

    match DefaultAccountService::import_account(&db, account_string) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountImportError::AccountStringCorrupted(_)
            | AccountImportError::AccountStringFailedToDeserialize(_)
            | AccountImportError::InvalidPrivateKey(_) => Err(AccountStringCorrupted),
            AccountImportError::PersistenceError(_) => {
                Err(ImportError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

pub enum GetAccountError {
    NoAccount,
    UnexpectedError(String),
}

pub fn get_account(config: &Config) -> Result<Account, GetAccountError> {
    let db = connect_to_db(&config).map_err(GetAccountError::UnexpectedError)?;

    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => Ok(account),
        Err(err) => match err {
            AccountRepoError::NoAccount(_) => Err(GetAccountError::NoAccount),
            AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                Err(GetAccountError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

pub enum CreateFileAtPathEnum {
    FileAlreadyExists,
    NoAccount,
    NoRoot,
    PathDoesntStartWithRoot,
    DocumentTreatedAsFolder,
    UnexpectedError(String),
}

pub fn create_file_at_path(
    config: &Config,
    path_and_name: &str,
) -> Result<FileMetadata, CreateFileAtPathEnum> {
    let db = connect_to_db(&config).map_err(CreateFileAtPathEnum::UnexpectedError)?;

    match DefaultFileService::create_at_path(&db, path_and_name) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            NewFileFromPathError::PathDoesntStartWithRoot => Err(PathDoesntStartWithRoot),
            NewFileFromPathError::FileAlreadyExists => Err(FileAlreadyExists),
            NewFileFromPathError::NoRoot => Err(NoRoot),
            NewFileFromPathError::FailedToCreateChild(failed_to_create) => match failed_to_create {
                NewFileError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount(_) => Err(CreateFileAtPathEnum::NoAccount),
                    AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                        CreateFileAtPathEnum::UnexpectedError(format!("{:#?}", account_error)),
                    ),
                },
                NewFileError::FileNameNotAvailable => Err(FileAlreadyExists),
                NewFileError::DocumentTreatedAsFolder => Err(DocumentTreatedAsFolder),
                NewFileError::CouldNotFindParents(_)
                | NewFileError::FileCryptoError(_)
                | NewFileError::MetadataRepoError(_)
                | NewFileError::FailedToWriteFileContent(_)
                | NewFileError::FailedToRecordChange(_)
                | NewFileError::FileNameContainsSlash => Err(
                    CreateFileAtPathEnum::UnexpectedError(format!("{:#?}", failed_to_create)),
                ),
            },
            NewFileFromPathError::FailedToRecordChange(_) | NewFileFromPathError::DbError(_) => {
                Err(CreateFileAtPathEnum::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}
