use crate::model::errors::core_err_unexpected;
use crate::pure_functions::files;
use crate::repo::schema::{OneKey, Tx};
use crate::service::{api_service, file_encryption_service};
use crate::CoreError;
use lockbook_crypto::clock_service::get_time;
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::{GetPublicKeyRequest, NewAccountRequest};

impl Tx<'_> {
    pub fn create_account(&mut self, username: &str, api_url: &str) -> Result<Account, CoreError> {
        let username = String::from(username).to_lowercase();

        if self.account.get(&OneKey {}).is_some() {
            return Err(CoreError::AccountExists);
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

        let root = file_encryption_service::encrypt_metadata(&account, &[root_metadata])?
            .first()
            .ok_or_else(|| CoreError::Unexpected("Failed to encrypt root".to_string()))?
            .clone();
        self.account.insert(OneKey {}, account.clone());
        self.base_metadata.insert(root.id, root.clone());
        self.last_synced.insert(OneKey {}, get_time().0);
        self.root.insert(OneKey {}, root.id);
        Ok(account)
    }

    pub fn import_account(&mut self, account_string: &str) -> Result<Account, CoreError> {
        if self.account.get(&OneKey {}).is_some() {
            warn!("tried to import an account, but account exists already.");
            return Err(CoreError::AccountExists);
        }

        let decoded = match base64::decode(&account_string) {
            Ok(d) => d,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted);
            }
        };

        let account: Account = match bincode::deserialize(&decoded[..]) {
            Ok(a) => a,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted);
            }
        };

        let server_public_key = api_service::request(
            &account,
            GetPublicKeyRequest { username: account.username.clone() },
        )?
        .key;

        if account.public_key() != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch);
        }

        self.account.insert(OneKey {}, account.clone());

        Ok(account)
    }

    pub fn export_account(&self) -> Result<String, CoreError> {
        let account = self
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
        Ok(base64::encode(&encoded))
    }

    pub fn get_account(&self) -> Result<Account, CoreError> {
        self.account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)
    }
}
