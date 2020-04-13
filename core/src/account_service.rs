use std::marker::PhantomData;

use crate::account::Account;
use crate::account_api;
use crate::account_repo;
use crate::account_repo::AccountRepo;
use crate::auth_service;
use crate::auth_service::AuthService;
use crate::crypto;
use crate::crypto::CryptoService;
use crate::db_provider;
use crate::db_provider::DbProvider;
use crate::error_enum;
use crate::lockbook_api::new_account;
use crate::lockbook_api::new_account::NewAccountClient;
use crate::lockbook_api::NewAccountRequest;
use crate::state::Config;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(crypto::KeyGenError),
        PersistenceError(account_repo::Error),
        ApiError(account_api::Error),
        AuthError(auth_service::AuthGenError),
        AccountGenerationError(new_account::NewAccountError)
    }
}

pub trait AccountService {
    fn create_account(config: Config, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<
    DB: DbProvider,
    Crypto: CryptoService,
    AccountDb: AccountRepo,
    Auth: AuthService,
    NewAccount: NewAccountClient,
> {
    db: PhantomData<DB>,
    encryption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    auth: PhantomData<Auth>,
    new_account: PhantomData<NewAccount>,
}

impl<
    DB: DbProvider,
    Crypto: CryptoService,
    AccountDb: AccountRepo,
    Auth: AuthService,
    NewAccount: NewAccountClient,
> AccountService
for AccountServiceImpl<
    DB,
    Crypto,
    AccountDb,
    Auth,
    NewAccount,
>
{
    fn create_account(config: Config, username: String) -> Result<Account, Error> {
        let db = DB::connect_to_db(config)?;
        let keys = Crypto::generate_key()?;
        let auth = Auth::generate_auth(&keys, &username)?;
        let account_req = NewAccountRequest {
            username: username.clone(),
            auth,
            pub_key_n: keys.public_key.n.clone(),
            pub_key_e: keys.public_key.e.clone(),
        };
        let account = Account { username, keys };


        AccountDb::insert_account(&db, &account)?;
        NewAccount::new_account(String::from(Config::get_auth_delay()), &account_req)?;

        Ok(account)
    }
}
