use std::marker::PhantomData;

use crate::client;
use crate::client::{Client, NewAccountRequest};
use crate::crypto::CryptoService;
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::{crypto, API_LOC};
use rusqlite::Connection;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(crypto::KeyGenError),
        PersistenceError(account_repo::Error),
        ApiError(client::ClientError)
    }
}

pub trait AccountService {
    fn create_account(db: &Connection, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<Crypto: CryptoService, AccountDb: AccountRepo, ApiClient: Client> {
    encyption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
}

impl<Crypto: CryptoService, AccountDb: AccountRepo, ApiClient: Client> AccountService
    for AccountServiceImpl<Crypto, AccountDb, ApiClient>
{
    fn create_account(db: &Connection, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account { username, keys };

        AccountDb::insert_account(&db, &account)?;
        let new_account_request = NewAccountRequest {
            username: format!("{}", &account.username),
            auth: "".to_string(),
            pub_key_n: format!("{}", &&account.keys.public_key.n.to_string()),
            pub_key_e: format!("{}", &account.keys.public_key.e.to_string()),
        };

        ApiClient::new_account(API_LOC.to_string(), &new_account_request)?;

        Ok(account)
    }
}
