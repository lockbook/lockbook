use crate::client;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::service::account_service::AccountCreationError::{
    AccountExistsAlready, AccountRepoError,
};
use crate::service::account_service::AccountImportError::{
    FailedToVerifyAccountServerSide, PublicKeyMismatch,
};
use crate::service::file_encryption_service::{FileEncryptionService, RootFolderCreationError};
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, NewAccountError, NewAccountRequest,
};

#[derive(Debug)]
pub enum AccountCreationError {
    AccountRepoError(account_repo::AccountRepoError),
    FolderError(RootFolderCreationError),
    MetadataRepoError(file_metadata_repo::DbError),
    ApiError(client::ApiError<NewAccountError>),
    KeySerializationError(serde_json::error::Error),
    AccountExistsAlready,
}

#[derive(Debug)]
pub enum AccountImportError {
    AccountStringCorrupted(base64::DecodeError),
    AccountStringFailedToDeserialize(bincode::Error),
    PersistenceError(account_repo::AccountRepoError),
    AccountRepoError(account_repo::AccountRepoError),
    FailedToVerifyAccountServerSide(client::ApiError<GetPublicKeyError>),
    PublicKeyMismatch,
    AccountExistsAlready,
}

#[derive(Debug)]
pub enum AccountExportError {
    AccountRetrievalError(account_repo::AccountRepoError),
    AccountStringFailedToSerialize(bincode::Error),
}

pub trait AccountService {
    fn create_account(
        config: &Config,
        username: &str,
        api_url: &str,
    ) -> Result<Account, AccountCreationError>;
    fn import_account(config: &Config, account_string: &str)
        -> Result<Account, AccountImportError>;
    fn export_account(config: &Config) -> Result<String, AccountExportError>;
}

pub struct AccountServiceImpl<
    AccountDb: AccountRepo,
    FileCrypto: FileEncryptionService,
    FileMetadata: FileMetadataRepo,
> {
    _accounts: AccountDb,
    _file_crypto: FileCrypto,
    _file_db: FileMetadata,
}

impl<AccountDb: AccountRepo, FileCrypto: FileEncryptionService, FileMetadata: FileMetadataRepo>
    AccountService for AccountServiceImpl<AccountDb, FileCrypto, FileMetadata>
{
    fn create_account(
        config: &Config,
        username: &str,
        api_url: &str,
    ) -> Result<Account, AccountCreationError> {
        info!("Checking if account already exists");
        if AccountDb::maybe_get_account(config)
            .map_err(AccountRepoError)?
            .is_some()
        {
            return Err(AccountExistsAlready);
        }

        info!("Creating new account for {}", username);

        info!("Generating Key...");
        let keys = pubkey::generate_key();

        let account = Account {
            username: String::from(username),
            api_url: api_url.to_string(),
            private_key: keys,
        };

        info!("Generating Root Folder");
        let mut file_metadata = FileCrypto::create_metadata_for_root_folder(&account)
            .map_err(AccountCreationError::FolderError)?;

        info!("Sending username & public key to server");
        let version = client::request(&account, NewAccountRequest::new(&account, &file_metadata))
            .map_err(AccountCreationError::ApiError)?
            .folder_metadata_version;
        info!("Account creation success!");

        file_metadata.metadata_version = version;
        file_metadata.content_version = version;

        FileMetadata::insert(config, &file_metadata)
            .map_err(AccountCreationError::MetadataRepoError)?;

        debug!(
            "{}",
            serde_json::to_string(&account).map_err(AccountCreationError::KeySerializationError)?
        );

        info!("Saving account locally");
        AccountDb::insert_account(config, &account)
            .map_err(AccountCreationError::AccountRepoError)?;

        Ok(account)
    }

    fn import_account(
        config: &Config,
        account_string: &str,
    ) -> Result<Account, AccountImportError> {
        info!("Checking if account already exists");
        if AccountDb::maybe_get_account(config)
            .map_err(AccountImportError::AccountRepoError)?
            .is_some()
        {
            return Err(AccountImportError::AccountExistsAlready);
        }

        info!("Importing account string: {}", &account_string);

        let decoded =
            base64::decode(&account_string).map_err(AccountImportError::AccountStringCorrupted)?;
        debug!("Key is valid base64 string");

        let account: Account = bincode::deserialize(&decoded[..])
            .map_err(AccountImportError::AccountStringFailedToDeserialize)?;
        debug!("Key was valid bincode");

        info!(
            "Checking this username, public_key pair exists at {}",
            account.api_url
        );
        let server_public_key = client::request(
            &account,
            GetPublicKeyRequest {
                username: account.username.clone(),
            },
        )
        .map_err(FailedToVerifyAccountServerSide)?
        .key;

        if account.public_key() != server_public_key {
            return Err(PublicKeyMismatch);
        }

        info!("Account String seems valid, saving now");
        AccountDb::insert_account(config, &account)
            .map_err(AccountImportError::PersistenceError)?;

        info!("Account imported successfully");
        Ok(account)
    }

    fn export_account(config: &Config) -> Result<String, AccountExportError> {
        let account =
            &AccountDb::get_account(config).map_err(AccountExportError::AccountRetrievalError)?;
        let encoded: Vec<u8> = bincode::serialize(&account)
            .map_err(AccountExportError::AccountStringFailedToSerialize)?;
        Ok(base64::encode(&encoded))
    }
}
