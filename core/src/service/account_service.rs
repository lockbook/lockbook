use hmdb::transaction::Transaction;

use lockbook_crypto::clock_service::get_time;
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::{GetPublicKeyRequest, NewAccountRequest};

use crate::model::errors::{core_err_unexpected, CreateAccountError};
use crate::model::state::Config;
use crate::pure_functions::files;
use crate::schema::{OneKey, Tx};
use crate::service::{api_service, file_encryption_service};
use crate::{AccountExportError, CoreError, Error, GetAccountError, ImportError, LbCore, UiError};

impl LbCore {
    pub fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>> {
        let username = String::from(username).to_lowercase();
        if self.db.account.get(&OneKey {})?.is_some() {
            return Err(CoreError::AccountExists.into());
        }

        let keys = pubkey::generate_key();

        let account = Account { username, api_url: api_url.to_string(), private_key: keys };

        let mut root_metadata = files::create_root(&account);
        let encrypted_metadata =
            file_encryption_service::encrypt_metadata(&account, &[root_metadata.clone()])?;
        let encrypted_metadatum = files::single_or(
            encrypted_metadata,
            CoreError::Unexpected(String::from(
                "create_account: multiple metadata decrypted from root",
            )),
        )?;

        root_metadata.metadata_version =
            api_service::request(&account, NewAccountRequest::new(&account, &encrypted_metadatum))?
                .folder_metadata_version;

        self.db.transaction(|tx| {
            let account = account.clone();
            let root =
                file_encryption_service::encrypt_metadata(&account, &[root_metadata.clone()])?
                    .first()
                    .ok_or_else(|| CoreError::Unexpected("Failed to encrypt root".to_string()))?
                    .clone();
            tx.account.insert(OneKey {}, account.clone());
            tx.base_metadata.insert(root.id, root);
            tx.last_synced.insert(OneKey {}, get_time().0);
            Ok(account)
        })?
    }

    pub fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportError>> {
        if self.db.account.get(&OneKey {})?.is_some() {
            warn!("tried to import an account, but account exists already.");
            return Err(CoreError::AccountExists.into());
        }

        let decoded = match base64::decode(&account_string) {
            Ok(d) => d,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted.into());
            }
        };

        let account: Account = match bincode::deserialize(&decoded[..]) {
            Ok(a) => a,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted.into());
            }
        };

        let server_public_key = api_service::request(
            &account,
            GetPublicKeyRequest { username: account.username.clone() },
        )?
        .key;

        if account.public_key() != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch.into());
        }

        self.db.account.insert(OneKey {}, account.clone())?;

        Ok(account)
    }

    pub fn export_account(&self) -> Result<String, Error<AccountExportError>> {
        let account = self
            .db
            .account
            .get(&OneKey {})?
            .ok_or(CoreError::AccountNonexistent)?;
        let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
        Ok(base64::encode(&encoded))
    }

    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        let account = self.db.transaction(|tx| tx.get_account())??;
        Ok(account)
    }
}

impl Tx<'_> {
    pub fn get_account(&self) -> Result<Account, CoreError> {
        self.account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)
    }
}

pub fn create_account(
    _config: &Config, _username: &str, _api_url: &str,
) -> Result<Account, CoreError> {
    todo!()
}

pub fn import_account(_config: &Config, _account_string: &str) -> Result<Account, CoreError> {
    todo!()
}

pub fn export_account(_config: &Config) -> Result<String, CoreError> {
    todo!()
}
