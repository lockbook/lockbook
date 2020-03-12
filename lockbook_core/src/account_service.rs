use std::marker::PhantomData;

use crate::account::Account;
use crate::account_api;
use crate::account_api::AccountApi;
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
        KeyGenerationError(crypto::KeyGenError),
        PersistenceError(account_repo::Error),
        ApiError(account_api::Error)
    }
}

pub trait AccountService {
    fn create_account(config: Config, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<
    DB: DbProvider,
    Crypto: CryptoService,
    AccountDb: AccountRepo,
    Api: AccountApi,
> {
    encyption: PhantomData<Crypto>,
    acounts: PhantomData<AccountDb>,
    db: PhantomData<DB>,
    api: PhantomData<Api>,
}

impl<DB: DbProvider, Crypto: CryptoService, AccountDb: AccountRepo, Api: AccountApi> AccountService
    for AccountServiceImpl<DB, Crypto, AccountDb, Api>
{
    fn create_account(config: Config, username: String) -> Result<Account, Error> {
        let db = DB::connect_to_db(config)?;
        let keys = Crypto::generate_key()?;
        let account = Account { username, keys };

        AccountDb::insert_account(&db, &account)?;
        Api::new_account(&account)?;

        Ok(account)
    }
}
