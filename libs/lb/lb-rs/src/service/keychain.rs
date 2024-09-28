use crate::{
    model::account::Account,
    model::errors::{CoreError, LbResult},
    Lb,
};
use libsecp256k1::PublicKey;
use tokio::sync::OnceCell;

#[derive(Default, Clone)]
pub struct Keychain {
    account: OnceCell<Account>,
    public_key: OnceCell<PublicKey>,
}

impl Lb {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.keychain
            .account
            .get()
            .ok_or_else(|| CoreError::AccountNonexistent.into())
    }

    pub fn get_pk(&self) -> LbResult<PublicKey> {
        self.keychain
            .public_key
            .get()
            .copied()
            .ok_or_else(|| CoreError::AccountNonexistent.into())
    }

    #[doc(hidden)]
    pub async fn cache_account(&self, account: Account) {
        let pk = account.public_key();
        self.keychain.account.set(account);
        self.keychain.public_key.set(pk);
    }
}
