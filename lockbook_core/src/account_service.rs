use std::marker::PhantomData;

use crate::account;
use crate::account::Account;
use crate::account_repo::AccountRepo;
use crate::crypto;
use crate::account_repo;
use crate::crypto::CryptoService;
use crate::error_enum;
use crate::state::Config;

error_enum! {
    enum Error {
        KeyGenerationError(crypto::Error),
        KeyComponentMissing(account::Error),
        PersistenceError(account_repo::Error)
    }
}

pub trait AccountService {
    fn create_account(config: Config, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<Crypto: CryptoService, Accounts: AccountRepo> {
    encyption: PhantomData<Crypto>,
    acounts: PhantomData<Accounts>,
}

impl<Crypto: CryptoService, Accounts: AccountRepo> AccountService
for AccountServiceImpl<Crypto, Accounts>
{
    fn create_account(config: Config, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account::new(username, keys)?;
        Accounts::insert_account(config, &account)?;
        Ok(account)
    }
}
