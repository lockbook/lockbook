use std::marker::PhantomData;

use crate::client;
use crate::client::{Client, NewAccountRequest};
use crate::crypto::PubKeyCryptoService;
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use sled::Db;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(rsa::errors::Error),
        PersistenceError(account_repo::Error),
        ApiError(client::ClientError),
        KeySerializationError(serde_json::error::Error),
    }
}

pub trait AccountService {
    fn create_account(db: &Db, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<
    Crypto: PubKeyCryptoService,
    AccountDb: AccountRepo,
    ApiClient: Client,
> {
    encryption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
}

impl<Crypto: PubKeyCryptoService, AccountDb: AccountRepo, ApiClient: Client> AccountService
    for AccountServiceImpl<Crypto, AccountDb, ApiClient>
{
    fn create_account(db: &Db, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account { username, keys };

        let username = account.username.clone();
        let auth = "".to_string();
        let public_key = serde_json::to_string(&account.keys.to_public_key())?;

        AccountDb::insert_account(&db, &account)?;
        let new_account_request = NewAccountRequest {
            username,
            auth,
            public_key,
        };

        ApiClient::new_account(&new_account_request)?;

        Ok(account)
    }
}
