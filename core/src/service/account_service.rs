use crate::model::errors::core_err_unexpected;
use crate::pure_functions::files;
use crate::repo::schema::OneKey;
use crate::service::{api_service, file_encryption_service};
use crate::{CoreError, RequestContext};
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_crypto::pubkey;
use lockbook_models::account::Account;
use lockbook_models::api::{GetPublicKeyRequest, NewAccountRequest};
use lockbook_models::tree::FileMetaMapExt;
use std::collections::HashMap;

impl RequestContext<'_, '_> {
    pub fn create_account(&mut self, username: &str, api_url: &str) -> Result<Account, CoreError> {
        let username = String::from(username).to_lowercase();

        if self.tx.account.get(&OneKey {}).is_some() {
            return Err(CoreError::AccountExists);
        }

        let keys = pubkey::generate_key();

        let account = Account { username, api_url: api_url.to_string(), private_key: keys };
        let public_key = account.public_key();
        self.data_cache.public_key = Some(public_key);

        let mut root_metadata = files::create_root(&account)?;
        let encrypted_metadata = file_encryption_service::encrypt_metadata(
            &account,
            &HashMap::with(root_metadata.clone()),
        )?;
        let encrypted_metadatum = if encrypted_metadata.len() == 1 {
            Ok(encrypted_metadata.into_values().next().unwrap())
        } else {
            Err(CoreError::Unexpected(String::from(
                "create_account: multiple metadata decrypted from root",
            )))
        }?;

        root_metadata.metadata_version =
            api_service::request(&account, NewAccountRequest::new(&account, &encrypted_metadatum))?
                .folder_metadata_version;

        let root = file_encryption_service::encrypt_metadata(
            &account,
            &HashMap::with(root_metadata.clone()),
        )?
        .get(&root_metadata.id)
        .ok_or_else(|| CoreError::Unexpected("Failed to encrypt root".to_string()))?
        .clone();
        self.tx.account.insert(OneKey {}, account.clone());
        self.tx.base_metadata.insert(root.id, root.clone());
        self.tx.last_synced.insert(OneKey {}, get_time().0);
        self.tx.root.insert(OneKey {}, root.id);
        Ok(account)
    }

    pub fn import_account(&mut self, account_string: &str) -> Result<Account, CoreError> {
        if self.tx.account.get(&OneKey {}).is_some() {
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

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch);
        }

        self.data_cache.public_key = Some(account_public_key);
        self.tx.account.insert(OneKey {}, account.clone());

        Ok(account)
    }

    pub fn export_account(&self) -> Result<String, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
        Ok(base64::encode(&encoded))
    }

    pub fn get_account(&self) -> Result<Account, CoreError> {
        self.tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)
    }

    pub fn get_public_key(&mut self) -> Result<PublicKey, CoreError> {
        match self.data_cache.public_key {
            Some(pk) => Ok(pk),
            None => {
                let account = self.get_account()?;
                let pk = account.public_key();
                self.data_cache.public_key = Some(pk);
                Ok(pk)
            }
        }
    }
}
