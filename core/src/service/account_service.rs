use std::marker::PhantomData;

use crate::client::{Client, NewAccountRequest};
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::service::crypto_service::PubKeyCryptoService;
use crate::{client, debug};
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
    fn load_account(db: &Db, username: String, key_string: String) -> Result<Account, Error>;
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
        let account = Account {
            username,
            keys: keys.clone(),
        };
        debug(format!("Keys: {:?}", serde_json::to_string(&keys).unwrap()));
        let username = account.username.clone();
        let auth = "".to_string();
        let public_key = serde_json::to_string(&account.keys.to_public_key())?;

        AccountDb::insert_account(db, &account)?;
        let new_account_request = NewAccountRequest {
            username,
            auth,
            public_key,
        };

        ApiClient::new_account(&new_account_request)?;

        Ok(account)
    }

    fn load_account(db: &Db, username: String, key_string: String) -> Result<Account, Error> {
        let keys = serde_json::from_str(key_string.as_str())?;
        let account = Account { username, keys };

        AccountDb::insert_account(db, &account)?;
        Ok(account)
    }
}
