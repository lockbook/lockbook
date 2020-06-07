use std::marker::PhantomData;

use crate::client;
use crate::client::Client;
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
        ApiError(client::new_account::Error),
        KeySerializationError(serde_json::error::Error),
        AuthGenFailure(AuthGenError)
    }
}

error_enum! {
    enum AccountImportError {
        AccountStringCorrupted(serde_json::error::Error),
        PersistenceError(account_repo::Error),
        InvalidPrivateKey(rsa::errors::Error),
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
            keys: keys,
        };
        let username = account.username.clone();
        let auth = Auth::generate_auth(&account)?;
        let public_key = serde_json::to_string(&account.keys.to_public_key())?;

        info!("Saving account locally");
        AccountDb::insert_account(db, &account)?;

        info!("Sending username & public key to server");
        ApiClient::new_account(username, auth, public_key)?;
        info!("Account creation success!");

        debug!("{}", serde_json::to_string(&account).unwrap());
        Ok(account)
    }

    fn import_account(db: &Db, account_string: &String) -> Result<Account, AccountImportError> {
        let account = serde_json::from_str::<Account>(account_string.as_str())?;
        account.keys.validate()?;
        AccountDb::insert_account(db, &account)?;
        info!("Account imported successfully");
        Ok(account)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::ClientImpl;
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::AccountRepoImpl;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::service::account_service::AccountImportError;
    use crate::service::account_service::{AccountService, AccountServiceImpl};
    use crate::service::auth_service::AuthServiceImpl;
    use crate::service::clock_service::ClockImpl;
    use crate::service::crypto_service::RsaImpl;
    use rsa::{BigUint, RSAPrivateKey};
    use std::mem::discriminant;

    type DefaultClock = ClockImpl;
    type DefaultCrypto = RsaImpl;
    type DefaultApiClient = ClientImpl;
    type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
    type DefaultAccountDb = AccountRepoImpl;
    type DefaultDbProvider = TempBackedDB;
    type DefaultAccountService =
        AccountServiceImpl<DefaultCrypto, DefaultAccountDb, DefaultApiClient, DefaultAuthService>;

    #[test]
    fn test_import_invalid_private_key() {
        let account = Account {
            username: "Smail".to_string(),
            keys: RSAPrivateKey::from_components(
                BigUint::from_bytes_be(b"Test"),
                BigUint::from_bytes_be(b"Test"),
                BigUint::from_bytes_be(b"Test"),
                vec![
                    BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
                    BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
                ],
            ),
        };
        let config = Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let result = discriminant(
            &DefaultAccountService::import_account(&db, &serde_json::to_string(&account).unwrap())
                .unwrap_err(),
        );
        let err = discriminant(&AccountImportError::InvalidPrivateKey(
            rsa::errors::Error::InvalidModulus,
        ));

        assert_eq!(result, err)
    }
}
