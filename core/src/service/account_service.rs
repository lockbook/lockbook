use std::marker::PhantomData;

use crate::client;
use crate::client::{Client, NewAccountRequest};
use crate::error_enum;
use crate::model::account::Account;
use crate::repo::account_repo;
use crate::repo::account_repo::AccountRepo;
use crate::service::auth_service::AuthGenError;
use crate::service::auth_service::AuthService;
use crate::service::crypto_service::PubKeyCryptoService;
use sled::Db;

error_enum! {
    enum AccountCreationError {
        KeyGenerationError(rsa::errors::Error),
        PersistenceError(account_repo::Error),
        ApiError(client::NewAccountError),
        KeySerializationError(serde_json::error::Error),
        AuthGenFailure(AuthGenError)
    }
}

error_enum! {
    enum AccountImportError {
        AccountStringCorrupted(serde_json::error::Error),
        PersistenceError(account_repo::Error),
    }
}

pub trait AccountService {
    fn create_account(db: &Db, username: &String) -> Result<Account, AccountCreationError>;
    fn import_account(db: &Db, account_string: &String) -> Result<Account, AccountImportError>;
}

pub struct AccountServiceImpl<
    Crypto: PubKeyCryptoService,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Auth: AuthService,
> {
    encryption: PhantomData<Crypto>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
    auth: PhantomData<Auth>,
}

impl<Crypto: PubKeyCryptoService, AccountDb: AccountRepo, ApiClient: Client, Auth: AuthService>
    AccountService for AccountServiceImpl<Crypto, AccountDb, ApiClient, Auth>
{
    fn create_account(db: &Db, username: &String) -> Result<Account, AccountCreationError> {
        info!("Creating new account for {}", username);

        info!("Generating Key...");
        let keys = Crypto::generate_key()?;

        let account = Account {
            username: username.clone(),
            keys: keys.clone(),
        };
        let username = account.username.clone();
        let auth = Auth::generate_auth(&account)?;
        let public_key = serde_json::to_string(&account.keys.to_public_key())?;

        info!("Saving account locally");
        AccountDb::insert_account(db, &account)?;

        let new_account_request = NewAccountRequest {
            username,
            auth,
            public_key,
        };

        info!("Sending username & public key to server");
        ApiClient::new_account(&new_account_request)?;
        info!("Account creation success!");

        info!("{}", serde_json::to_string(&account).unwrap());
        Ok(account)
    }

    fn import_account(db: &Db, account_string: &String) -> Result<Account, AccountImportError> {
        let account = serde_json::from_str(account_string.as_str())?;

        AccountDb::insert_account(db, &account)?;
        info!("Account imported successfully");
        Ok(account)
    }
}
