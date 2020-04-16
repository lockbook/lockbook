use std::marker::PhantomData;

use crate::client;
use crate::client::{Client, NewAccountRequest};
use crate::crypto::PubKeyCryptoService;
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::API_LOC;
use sled::Db;
use crate::auth_service::AuthService;
use crate::auth_service::AuthGenError;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(rsa::errors::Error),
        PersistenceError(account_repo::Error),
        ApiError(client::ClientError),
        KeySerializationError(serde_json::error::Error),
        AuthGenFailure(AuthGenError)
    }
}

pub trait AccountService {
    fn create_account(db: &Db, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<
    Crypto: PubKeyCryptoService,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Auth: AuthService
> {
    encryption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
    auth: PhantomData<Auth>
}

impl<Crypto: PubKeyCryptoService, AccountDb: AccountRepo, ApiClient: Client, Auth: AuthService> AccountService
    for AccountServiceImpl<Crypto, AccountDb, ApiClient, Auth>
{
    fn create_account(db: &Db, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account { username: username, keys: keys.clone() };

        let username = account.username.clone();
        let auth = Auth::generate_auth(&keys, &username)?;
        let public_key = serde_json::to_string(&account.keys.to_public_key())?;

        AccountDb::insert_account(&db, &account)?;
        let new_account_request = NewAccountRequest {
            username,
            auth,
            public_key,
        };

        ApiClient::new_account(API_LOC.to_string(), &new_account_request)?;

        Ok(account)
    }
}
