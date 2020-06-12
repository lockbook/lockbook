use std::marker::PhantomData;

use crate::client;
use crate::client::Client;
use crate::error_enum;
use crate::model::account::Account;
use crate::model::api::NewAccountError;
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
        ApiError(client::Error<NewAccountError>),
        KeySerializationError(serde_json::error::Error),
        AuthGenFailure(AuthGenError)
    }
}

error_enum! {
    enum AccountImportError {
        AccountStringCorrupted(base64::DecodeError),
        AccountStringFailedToDeserialize(bincode::Error),
        PersistenceError(account_repo::Error),
        InvalidPrivateKey(rsa::errors::Error),
    }
}

error_enum! {
    enum AccountExportError {
        KeyRetrievalError(account_repo::Error),
        AccountStringFailedToSerialize(bincode::Error),
    }
}

pub trait AccountService {
    fn create_account(db: &Db, username: &String) -> Result<Account, AccountCreationError>;
    fn import_account(db: &Db, account_string: &String) -> Result<Account, AccountImportError>;
    fn export_account(db: &Db) -> Result<String, AccountExportError>;
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

        info!("Saving account locally");
        AccountDb::insert_account(db, &account)?;

        info!("Sending username & public key to server");
        ApiClient::new_account(username, auth, account.keys.to_public_key())?;
        info!("Account creation success!");

        debug!("{}", serde_json::to_string(&account)?);
        Ok(account)
    }

    fn import_account(db: &Db, account_string: &String) -> Result<Account, AccountImportError> {
        info!("Importing account string: {}", &account_string);

        let decoded = base64::decode(&account_string)?;
        debug!("Key is valid base64 string");

        let account: Account = bincode::deserialize(&decoded[..])?;
        debug!("Key was valid bincode");

        account.keys.validate()?;
        debug!("RSA says the key is valid");

        info!("Account String seems valid, saving now");
        AccountDb::insert_account(db, &account)?;

        info!("Account imported successfully");
        Ok(account)
    }

    fn export_account(db: &Db) -> Result<String, AccountExportError> {
        let account = &AccountDb::get_account(&db)?;
        let encoded: Vec<u8> = bincode::serialize(&account)?;
        Ok(base64::encode(&encoded))
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::ClientImpl;
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
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
        DefaultAccountDb::insert_account(&db, &account).unwrap();

        let result = discriminant(
            &DefaultAccountService::import_account(
                &db,
                &DefaultAccountService::export_account(&db).unwrap(),
            )
            .unwrap_err(),
        );
        let err = discriminant(&AccountImportError::InvalidPrivateKey(
            rsa::errors::Error::InvalidModulus,
        ));

        assert_eq!(result, err)
    }

    #[test]
    fn test_import_export_opposites() {
        let account_string = "BgAAAAAAAABwYXJ0aDRAAAAAAAAAAJnSeo+j1kZ6zi/Sfw/k6h8adzTImXng3ZXqvSKOUyYatb1Xm7Kh3AFPNSkTytGC/3ajran8/WhUnImJobEg0MGQoXdLiuwxtMs45RhuSDlBPPwW+Dw8EUt3ElEkgMkXXsZzcIfOSuTxTh+pmJWJJO5v4tyTu0jhXP7WJ9yK44EzQUpWVwTLb4t81wuUU5tJ/f4ybr/UrRmjXSLqKybUdjRQseF4l+aH8Ony3yC93UhlNlZtInoJIZCa+xuoJQsPHM+lzdZcHi3GhAw3t8BSnP5oW/j+mnRbb/h67RRqb+C+7b+x4ixrliCO0ekEhC/W0VhymZQh0YYMb7X/Vm6nSLoBAAAAAAAAAAEAAQBAAAAAAAAAAI1X0y8br/ltxnEYZxfO/6TLorOKEJd5H/0XeDXDiMjSvSPOCzuCbhSGWQVPdU9iegHdCHOrqA21pcSfJ5c2+0I38HRpWYZeQk2ochDTqqe23WJ27kt5CgrK6gXG5MeROCSEMSiJwcelhkdVYf5bSsdqGi681T4416lravO07oSTggy/dw/+w/BcYWXEjN07ujYgt4zOkYBQ4C1t3bVRAjEnx6EkF4UOHxlcbIbdfD/Txmm9AAhIz9MxQLq25U57bK5hoK6orOxxUMIZnpqvy9TH2+AZD2l9HjylVN2wC6gXLfIrPk0NUroxXVRcYuPhkCkvoWtq5bdW++1j5bRxAF4CAAAAAAAAACAAAAAAAAAAhx6QHKVxtuz2yNfzPOb5fJWZmuRWyDFzyrOQXFK7Q3o30iDtP+6AaQuRFX/75N6PDFJfjE/kHobsLd+yhNkDg19EkFM4dceKoR9WylGb3S2QmD9J7ew63EnPMs+mHqBqv1bsgh8+eTwo8teqA0oFSMz0OzwGRz0xn5jzmwZxKcwgAAAAAAAAAN+t+ahUxaKA8d5UDLjzjnxheC/QuneQAJVYDxExP+/9uchnBt1rxYiqBHWgaFiIHgAyfkaak4oFNZ+Cnf/Gb0qjHWGiF/f8/63rmv54XmfbpMifUNYnUSBSbEGU8KNRw1BZpofmadY6KfDV/aoyBUSX7yU9rPT9hbkpjR5oIpXp".to_string();
        let config = Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        DefaultAccountService::import_account(&db, &account_string).unwrap();
        assert_eq!(
            DefaultAccountService::export_account(&db).unwrap(),
            account_string
        );
    }
}
