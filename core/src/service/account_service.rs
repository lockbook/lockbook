use crate::model::errors::core_err_unexpected;
use crate::repo::schema::OneKey;
use crate::service::api_service;
use crate::{CoreError, CoreResult, RequestContext};
use libsecp256k1::PublicKey;
use lockbook_shared::account::Account;
use lockbook_shared::api::{GetPublicKeyRequest, NewAccountRequest};
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileMetadata;
use lockbook_shared::pubkey;
use lockbook_shared::server_file::IntoServerFile;

impl RequestContext<'_, '_> {
    pub fn create_account(&mut self, username: &str, api_url: &str) -> CoreResult<Account> {
        let username = String::from(username).to_lowercase();

        if self.tx.account.get(&OneKey {}).is_some() {
            return Err(CoreError::AccountExists);
        }

        let private_key = pubkey::generate_key();

        let account = Account { username, api_url: api_url.to_string(), private_key };
        let public_key = account.public_key();
        self.data_cache.public_key = Some(public_key);

        let root = FileMetadata::create_root(&account)?.sign(&account)?;

        let version = api_service::request(&account, NewAccountRequest::new(&account, &root))?
            .folder_metadata_version;

        let root = root.add_time(version);
        let root_id = root.id();

        self.tx.account.insert(OneKey {}, account.clone());
        self.tx.base_metadata.insert(root_id, root);
        self.tx.last_synced.insert(OneKey {}, get_time().0);
        self.tx.root.insert(OneKey {}, root_id);
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

    pub fn get_account(&self) -> Result<&Account, CoreError> {
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
