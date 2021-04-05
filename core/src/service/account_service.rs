use crate::client;
use crate::client::Client;
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
use crate::service::crypto_service::PubKeyCryptoService;
use crate::service::file_encryption_service::{FileEncryptionService, RootFolderCreationError};
use crate::storage::db_provider::Backend;
use lockbook_models::account::Account;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, NewAccountError, NewAccountRequest,
};

#[derive(Debug)]
pub enum AccountCreationError<MyBackend: Backend> {
    KeyGenerationError(rsa::errors::Error),
    AccountRepoError(account_repo::AccountRepoError<MyBackend>),
    FolderError(RootFolderCreationError),
    MetadataRepoError(file_metadata_repo::DbError<MyBackend>),
    ApiError(client::ApiError<NewAccountError>),
    KeySerializationError(serde_json::error::Error),
    AccountExistsAlready,
}

#[derive(Debug)]
pub enum AccountImportError<MyBackend: Backend> {
    AccountStringCorrupted(base64::DecodeError),
    AccountStringFailedToDeserialize(bincode::Error),
    PersistenceError(account_repo::AccountRepoError<MyBackend>),
    InvalidPrivateKey(rsa::errors::Error),
    AccountRepoError(account_repo::AccountRepoError<MyBackend>),
    FailedToVerifyAccountServerSide(client::ApiError<GetPublicKeyError>),
    PublicKeyMismatch,
    AccountExistsAlready,
}

#[derive(Debug)]
pub enum AccountExportError<MyBackend: Backend> {
    AccountRetrievalError(account_repo::AccountRepoError<MyBackend>),
    AccountStringFailedToSerialize(bincode::Error),
}

pub trait AccountService<MyBackend: Backend> {
    fn create_account(
        backend: &MyBackend::Db,
        username: &str,
        api_url: &str,
    ) -> Result<Account, AccountCreationError<MyBackend>>;
    fn import_account(
        backend: &MyBackend::Db,
        account_string: &str,
    ) -> Result<Account, AccountImportError<MyBackend>>;
    fn export_account(backend: &MyBackend::Db) -> Result<String, AccountExportError<MyBackend>>;
}

pub struct AccountServiceImpl<
    Crypto: PubKeyCryptoService,
    AccountDb: AccountRepo<MyBackend>,
    ApiClient: Client,
    FileCrypto: FileEncryptionService,
    FileMetadata: FileMetadataRepo<MyBackend>,
    MyBackend: Backend,
> {
    _encryption: Crypto,
    _accounts: AccountDb,
    _client: ApiClient,
    _file_crypto: FileCrypto,
    _file_db: FileMetadata,
    _backend: MyBackend,
}

impl<
        Crypto: PubKeyCryptoService,
        AccountDb: AccountRepo<MyBackend>,
        ApiClient: Client,
        FileCrypto: FileEncryptionService,
        FileMetadata: FileMetadataRepo<MyBackend>,
        MyBackend: Backend,
    > AccountService<MyBackend>
    for AccountServiceImpl<Crypto, AccountDb, ApiClient, FileCrypto, FileMetadata, MyBackend>
{
    fn create_account(
        backend: &MyBackend::Db,
        username: &str,
        api_url: &str,
    ) -> Result<Account, AccountCreationError<MyBackend>> {
        info!("Checking if account already exists");
        if AccountDb::maybe_get_account(backend)
            .map_err(AccountRepoError)?
            .is_some()
        {
            return Err(AccountExistsAlready);
        }

        info!("Creating new account for {}", username);

        info!("Generating Key...");
        let keys = Crypto::generate_key().map_err(AccountCreationError::KeyGenerationError)?;

        let account = Account {
            username: String::from(username),
            api_url: api_url.to_string(),
            private_key: keys,
        };

        info!("Generating Root Folder");
        let mut file_metadata = FileCrypto::create_metadata_for_root_folder(&account)
            .map_err(AccountCreationError::FolderError)?;

        info!("Sending username & public key to server");
        let version =
            ApiClient::request(&account, NewAccountRequest::new(&account, &file_metadata))
                .map_err(AccountCreationError::ApiError)?
                .folder_metadata_version;
        info!("Account creation success!");

        file_metadata.metadata_version = version;
        file_metadata.content_version = version;

        FileMetadata::insert(backend, &file_metadata)
            .map_err(AccountCreationError::MetadataRepoError)?;

        debug!(
            "{}",
            serde_json::to_string(&account).map_err(AccountCreationError::KeySerializationError)?
        );

        info!("Saving account locally");
        AccountDb::insert_account(backend, &account)
            .map_err(AccountCreationError::AccountRepoError)?;

        Ok(account)
    }

    fn import_account(
        backend: &MyBackend::Db,
        account_string: &str,
    ) -> Result<Account, AccountImportError<MyBackend>> {
        info!("Checking if account already exists");
        if AccountDb::maybe_get_account(backend)
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

        account
            .private_key
            .validate()
            .map_err(AccountImportError::InvalidPrivateKey)?;
        debug!("RSA says the key is valid");

        info!(
            "Checking this username, public_key pair exists at {}",
            account.api_url
        );
        let server_public_key = ApiClient::request(
            &account,
            GetPublicKeyRequest {
                username: account.username.clone(),
            },
        )
        .map_err(FailedToVerifyAccountServerSide)?
        .key;
        if account.private_key.to_public_key() != server_public_key {
            return Err(PublicKeyMismatch);
        }

        info!("Account String seems valid, saving now");
        AccountDb::insert_account(backend, &account)
            .map_err(AccountImportError::PersistenceError)?;

        info!("Account imported successfully");
        Ok(account)
    }

    fn export_account(backend: &MyBackend::Db) -> Result<String, AccountExportError<MyBackend>> {
        let account =
            &AccountDb::get_account(backend).map_err(AccountExportError::AccountRetrievalError)?;
        let encoded: Vec<u8> = bincode::serialize(&account)
            .map_err(AccountExportError::AccountStringFailedToSerialize)?;
        Ok(base64::encode(&encoded))
    }
}
