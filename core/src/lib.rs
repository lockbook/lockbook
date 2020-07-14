extern crate reqwest;

#[macro_use]
extern crate log;
use crate::client::Error::SendFailed;
use crate::client::{ClientImpl, Error};
use crate::model::account::Account;
use crate::model::api::NewAccountError;
use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoImpl;
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::document_repo::DocumentRepoImpl;
use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{AccountCreationError, AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::FileServiceImpl;
use crate::service::sync_service::FileSyncService;
use crate::CreateAccountError::{
    CouldNotReachServer, InvalidUsername, UnexpectedError, UsernameTaken,
};
pub use sled::Db;

pub mod c_interface;
pub mod client;
pub mod model;
pub mod repo;
pub mod service;

mod java_interface;

static API_URL: &str = env!("API_URL");
static DB_NAME: &str = "lockbook.sled";

static INTERNAL_ERROR: &str = "Internal Error";

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
