use crate::client;
use crate::client::ApiError;
use crate::core_err_unexpected;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::file_metadata_repo;
use crate::service::file_encryption_service;
use crate::CoreError;
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, NewAccountError, NewAccountRequest,
};

pub fn create_account(
    config: &Config,
    username: &str,
    api_url: &str,
) -> Result<Account, CoreError> {
    info!("Checking if account already exists");
    if account_repo::maybe_get_account(config)?.is_some() {
        return Err(CoreError::AccountExists);
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
    let mut file_metadata = file_encryption_service::create_metadata_for_root_folder(&account)?;

    info!("Sending username & public key to server");
    let version = match client::request(&account, NewAccountRequest::new(&account, &file_metadata))
    {
        Ok(response) => response.folder_metadata_version,
        Err(ApiError::SendFailed(_)) => {
            return Err(CoreError::ServerUnreachable);
        }
        Err(ApiError::ClientUpdateRequired) => {
            return Err(CoreError::ClientUpdateRequired);
        }
        Err(ApiError::Endpoint(NewAccountError::UsernameTaken)) => {
            return Err(CoreError::UsernameTaken);
        }
        Err(ApiError::Endpoint(NewAccountError::InvalidUsername)) => {
            return Err(CoreError::UsernameInvalid);
        }
        Err(e) => {
            return Err(core_err_unexpected(e));
        }
    };
    info!("Account creation success!");

    file_metadata.metadata_version = version;
    file_metadata.content_version = version;

    file_metadata_repo::insert(config, &file_metadata)?;

    debug!(
        "{}",
        serde_json::to_string(&account).map_err(core_err_unexpected)?
    );

    info!("Saving account locally");
    account_repo::insert_account(config, &account)?;

    Ok(account)
}

pub fn import_account(config: &Config, account_string: &str) -> Result<Account, CoreError> {
    info!("Checking if account already exists");
    if account_repo::maybe_get_account(config)?.is_some() {
        return Err(CoreError::AccountExists);
    }

    info!("Importing account string: {}", &account_string);

    let decoded = match base64::decode(&account_string) {
        Ok(d) => d,
        Err(_) => {
            return Err(CoreError::AccountStringCorrupted);
        }
    };
    debug!("Key is valid base64 string");

    let account: Account = match bincode::deserialize(&decoded[..]) {
        Ok(a) => a,
        Err(_) => {
            return Err(CoreError::AccountStringCorrupted);
        }
    };
    debug!("Key was valid bincode");

    info!(
        "Checking this username, public_key pair exists at {}",
        account.api_url
    );
    let server_public_key = match client::request(
        &account,
        GetPublicKeyRequest {
            username: account.username.clone(),
        },
    ) {
        Ok(response) => response.key,
        Err(ApiError::SendFailed(_)) => {
            return Err(CoreError::ServerUnreachable);
        }
        Err(ApiError::ClientUpdateRequired) => {
            return Err(CoreError::ClientUpdateRequired);
        }
        Err(ApiError::Endpoint(GetPublicKeyError::UserNotFound)) => {
            return Err(CoreError::AccountNonexistent);
        }
        Err(e) => {
            return Err(core_err_unexpected(e));
        }
    };

    if account.public_key() != server_public_key {
        return Err(CoreError::UsernamePublicKeyMismatch);
    }

    info!("Account String seems valid, saving now");
    account_repo::insert_account(config, &account)?;

    info!("Account imported successfully");
    Ok(account)
}

pub fn export_account(config: &Config) -> Result<String, CoreError> {
    let account = account_repo::get_account(config)?;
    let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
    Ok(base64::encode(&encoded))
}
