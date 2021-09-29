use crate::client::ApiError;
use crate::core_err_unexpected;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::service::{file_encryption_service, file_service, sync_service};
use crate::CoreError;
use crate::{client, utils};
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
    if account_repo::maybe_get(config)?.is_some() {
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
    let root_metadata = file_service::create_root(&account.username);
    let encrypted_metadata =
        file_encryption_service::encrypt_metadata(&account, &[root_metadata.clone()])?;
    let encrypted_metadatum = utils::single_or(
        encrypted_metadata,
        CoreError::Unexpected(String::from(
            "create_account: multiple metadata decrypted from root",
        )),
    )?;

    info!("Sending username & public key to server");
    match client::request(
        &account,
        NewAccountRequest::new(&account, &encrypted_metadatum),
    ) {
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

    debug!(
        "{}",
        serde_json::to_string(&account).map_err(core_err_unexpected)?
    );

    info!("Saving account locally");
    account_repo::insert(config, &account)?;

    info!("Performing initial sync");
    sync_service::sync(config, None)?;

    Ok(account)
}

pub fn import_account(config: &Config, account_string: &str) -> Result<Account, CoreError> {
    info!("Checking if account already exists");
    if account_repo::maybe_get(config)?.is_some() {
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
    account_repo::insert(config, &account)?;

    info!("Performing initial sync");
    sync_service::sync(config, None)?;

    info!("Account imported successfully");
    Ok(account)
}

pub fn export_account(config: &Config) -> Result<String, CoreError> {
    let account = account_repo::get(config)?;
    let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
    Ok(base64::encode(&encoded))
}
