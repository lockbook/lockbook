use std::marker::PhantomData;

use crate::client;
use crate::client::{Client, NewAccountRequest};
use crate::crypto;
use crate::crypto::CryptoService;
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use sled::Db;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        KeyGenerationError(crypto::KeyGenError),
        PersistenceError(account_repo::Error),
        ApiError(client::ClientError)
    }
}

pub trait AccountService {
    fn create_account(db: &Db, username: String) -> Result<Account, Error>;
}

pub struct AccountServiceImpl<Crypto: CryptoService, AccountDb: AccountRepo, ApiClient: Client> {
    encyption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
}

impl<Crypto: CryptoService, AccountDb: AccountRepo, ApiClient: Client> AccountService
    for AccountServiceImpl<Crypto, AccountDb, ApiClient>
{
    fn create_account(db: &Db, username: String) -> Result<Account, Error> {
        let keys = Crypto::generate_key()?;
        let account = Account { username, keys };

        AccountDb::insert_account(&db, &account)?;

        ApiClient::new_account(&NewAccountRequest {
            username: format!("{}", &account.username),
            // FIXME: Real auth...
            auth: "JUNKAUTH".to_string(),
            pub_key_n: format!("{}", &&account.keys.public_key.n.to_string()),
            pub_key_e: format!("{}", &account.keys.public_key.e.to_string()),
        })?;

        Ok(account)
    }
}
