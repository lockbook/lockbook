use std::marker::PhantomData;

use crate::account;
use crate::account::Account;
use crate::account_repo;
use crate::account_repo::AccountRepo;
use crate::crypto;
use crate::crypto::CryptoService;
use crate::db_provider;
use crate::db_provider::DbProvider;
use crate::error_enum;
use crate::state::Config;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(crypto::Error),
        KeyComponentMissing(account::Error),
        PersistenceError(account_repo::Error)
    }
}

pub trait AccountService {
    fn create_account(config: Config, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<DB: DbProvider, Crypto: CryptoService, Accounts: AccountRepo> {
    encyption: PhantomData<Crypto>,
    acounts: PhantomData<Accounts>,
    db: PhantomData<DB>,
}

impl<DB: DbProvider, Crypto: CryptoService, Accounts: AccountRepo> AccountService
for AccountServiceImpl<DB, Crypto, Accounts>
{
    fn create_account(config: Config, username: String) -> Result<Account, Error> {
        let db = DB::connect_to_db(config)?;
        let keys = Crypto::generate_key()?;
        let account = Account::new(username, keys)?;
        Accounts::insert_account(&db, &account)?;
        Ok(account)
    }
}
