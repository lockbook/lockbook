use crate::model::errors::core_err_unexpected;
use crate::OneKey;
use crate::{CoreError, CoreResult, RequestContext, Requester};
use libsecp256k1::PublicKey;
use lockbook_shared::account::Account;
use lockbook_shared::api::{GetPublicKeyRequest, NewAccountRequest};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileMetadata;
use qrcode_generator::QrCodeEcc;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn create_account(&mut self, username: &str, api_url: &str) -> CoreResult<Account> {
        let username = String::from(username).to_lowercase();

        if self.tx.account.get(&OneKey {}).is_some() {
            return Err(CoreError::AccountExists);
        }

        let account = Account::new(username, api_url.to_string());
        let public_key = account.public_key();
        self.data_cache.public_key = Some(public_key);

        let root = FileMetadata::create_root(&account)?.sign(&account)?;

        let last_synced = self
            .client
            .request(&account, NewAccountRequest::new(&account, &root))?
            .last_synced;

        let root_id = *root.id();

        self.tx.account.insert(OneKey {}, account.clone());
        self.tx.base_metadata.insert(root_id, root);
        self.tx.last_synced.insert(OneKey {}, last_synced as i64);
        self.tx.root.insert(OneKey {}, root_id);
        Ok(account)
    }

    pub fn import_account(&mut self, account_string: &str) -> CoreResult<Account> {
        if self.tx.account.get(&OneKey {}).is_some() {
            warn!("tried to import an account, but account exists already.");
            return Err(CoreError::AccountExists);
        }

        let decoded = match base64::decode(account_string) {
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

        let server_public_key = self
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() })?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch);
        }

        self.data_cache.public_key = Some(account_public_key);
        self.tx.account.insert(OneKey {}, account.clone());

        Ok(account)
    }

    pub fn export_account(&self) -> CoreResult<String> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
        Ok(base64::encode(encoded))
    }

    pub fn export_account_qr(&self) -> CoreResult<Vec<u8>> {
        let acct_secret = self.export_account()?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(core_err_unexpected)
    }

    pub fn get_account(&self) -> CoreResult<&Account> {
        self.tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)
    }

    pub fn get_public_key(&mut self) -> CoreResult<PublicKey> {
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
