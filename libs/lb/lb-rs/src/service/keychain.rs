use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::Lb;
use crate::model::account::Account;
use crate::model::crypto::AESKey;
use crate::model::errors::{LbErrKind, LbResult};
use libsecp256k1::PublicKey;
use tokio::sync::OnceCell;
use uuid::Uuid;

pub type KeyCache = Arc<RwLock<HashMap<Uuid, AESKey>>>;

#[derive(Default, Clone)]
pub struct Keychain {
    key_cache: KeyCache,
    account: Arc<OnceCell<Account>>,
    public_key: Arc<OnceCell<PublicKey>>,
}

impl From<Option<&Account>> for Keychain {
    fn from(value: Option<&Account>) -> Self {
        match value {
            Some(account) => {
                let account = account.clone();
                let pk = account.public_key();
                let key_cache = Default::default();

                Self {
                    account: Arc::new(OnceCell::from(account)),
                    public_key: Arc::new(OnceCell::from(pk)),
                    key_cache,
                }
            }
            None => Self::default(),
        }
    }
}

impl Lb {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.keychain.get_account()
    }
}

impl Keychain {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.account
            .get()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    pub fn get_pk(&self) -> LbResult<PublicKey> {
        self.public_key
            .get()
            .copied()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    #[doc(hidden)]
    pub async fn cache_account(&self, account: Account) -> LbResult<()> {
        let pk = account.public_key();
        self.account
            .set(account)
            .map_err(|_| LbErrKind::AccountExists)?;
        self.public_key
            .set(pk)
            .map_err(|_| LbErrKind::AccountExists)?;

        Ok(())
    }

    pub fn contains_aes_key(&self, id: &Uuid) -> LbResult<bool> {
        Ok(self.key_cache.read()?.contains_key(id))
    }

    pub fn insert_aes_key(&self, id: Uuid, key: AESKey) -> LbResult<()> {
        self.key_cache.write()?.insert(id, key);
        Ok(())
    }

    pub fn get_aes_key(&self, id: &Uuid) -> LbResult<Option<AESKey>> {
        Ok(self.key_cache.read()?.get(id).copied())
    }
}
