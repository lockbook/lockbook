use std::marker::PhantomData;

use crate::account::account_repo;
use crate::account::account_repo::AccountRepo;
use crate::account_api;
use crate::account_api::AccountApi;
use crate::crypto;
use crate::crypto::CryptoService;
use crate::db_provider;
use crate::error_enum;
use crate::models::account::Account;
use rusqlite::Connection;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(crypto::KeyGenError),
        PersistenceError(account_repo::Error),
        ApiError(account_api::Error)
    }
}

pub trait AccountService {
    fn create_account(db: &Connection, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<Crypto: CryptoService, AccountDb: AccountRepo, Api: AccountApi> {
    encyption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    api: PhantomData<Api>,
}

impl<Crypto: CryptoService, AccountDb: AccountRepo, Api: AccountApi> AccountService
    for AccountServiceImpl<Crypto, AccountDb, Api>
{
    fn create_account(db: &Connection, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account { username, keys };

        AccountDb::insert_account(&db, &account)?;
        Api::new_account(&account)?;

        Ok(account)
    }
}
