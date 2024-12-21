use std::sync::Arc;

use crate::{
    model::account::Account,
    model::errors::{LbErrKind, LbResult},
    Lb,
};
use libsecp256k1::PublicKey;
use tokio::sync::OnceCell;

#[derive(Default, Clone)]
pub struct Keychain {
    account: Arc<OnceCell<Account>>,
    public_key: Arc<OnceCell<PublicKey>>,
}

impl From<Option<&Account>> for Keychain {
    fn from(value: Option<&Account>) -> Self {
        match value {
            Some(account) => {
                let account = account.clone();
                let pk = account.public_key();

                Self {
                    account: Arc::new(OnceCell::from(account)),
                    public_key: Arc::new(OnceCell::from(pk)),
                }
            }
            None => Self::default(),
        }
    }
}

impl Lb {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.keychain
            .account
            .get()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    pub fn get_pk(&self) -> LbResult<PublicKey> {
        self.keychain
            .public_key
            .get()
            .copied()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    #[doc(hidden)]
    pub async fn cache_account(&self, account: Account) -> LbResult<()> {
        let pk = account.public_key();
        self.keychain
            .account
            .set(account)
            .map_err(|_| LbErrKind::AccountExists)?;
        self.keychain
            .public_key
            .set(pk)
            .map_err(|_| LbErrKind::AccountExists)?;

        Ok(())
    }
}
