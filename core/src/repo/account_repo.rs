use crate::model::account::{Account, ApiUrl};
use crate::repo::account_repo::AccountRepoError::NoAccount;
use crate::storage::db_provider;
use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum AccountRepoError {
    BackendError(db_provider::BackendError),
    SerdeError(serde_json::Error),
    NoAccount,
}

pub trait AccountRepo {
    fn insert_account(backend: &Backend, account: &Account) -> Result<(), AccountRepoError>;
    fn maybe_get_account(backend: &Backend) -> Result<Option<Account>, AccountRepoError>;
    fn get_account(backend: &Backend) -> Result<Account, AccountRepoError>;
    fn get_api_url(backend: &Backend) -> Result<ApiUrl, AccountRepoError>;
}

pub struct AccountRepoImpl;

static ACCOUNT: &str = "account";
static YOU: &str = "you";

impl AccountRepo for AccountRepoImpl {
    fn insert_account(backend: &Backend, account: &Account) -> Result<(), AccountRepoError> {
        backend
            .write(
                ACCOUNT,
                YOU,
                serde_json::to_vec(account).map_err(AccountRepoError::SerdeError)?,
            )
            .map_err(AccountRepoError::BackendError)
    }

    fn maybe_get_account(backend: &Backend) -> Result<Option<Account>, AccountRepoError> {
        match Self::get_account(backend) {
            Ok(account) => Ok(Some(account)),
            Err(err) => match err {
                AccountRepoError::NoAccount => Ok(None),
                other => Err(other),
            },
        }
    }

    fn get_account(backend: &Backend) -> Result<Account, AccountRepoError> {
        let maybe_value: Option<Vec<u8>> = backend
            .read(ACCOUNT, YOU)
            .map_err(AccountRepoError::BackendError)?;
        match maybe_value {
            None => Err(NoAccount),
            Some(account) => {
                Ok(serde_json::from_slice(account.as_ref())
                    .map_err(AccountRepoError::SerdeError)?)
            }
        }
    }

    fn get_api_url(backend: &Backend) -> Result<ApiUrl, AccountRepoError> {
        Self::get_account(backend).map(|account| account.api_url)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::account::Account;
    use crate::model::state::temp_config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::service::clock_service::ClockImpl;
    use crate::service::crypto_service::{PubKeyCryptoService, RSAImpl};
    use crate::storage::db_provider::{to_backend, DbProvider, DiskBackedDB};

    type DefaultDbProvider = DiskBackedDB;
    type DefaultAccountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            api_url: "ftp://uranus.net".to_string(),
            private_key: RSAImpl::<ClockImpl>::generate_key().expect("Key generation failure"),
        };

        let config = temp_config();
        let db = &DefaultDbProvider::connect_to_db(&config).unwrap();
        let backend = &to_backend(db);
        let res = DefaultAccountRepo::get_account(backend);
        assert!(res.is_err());

        DefaultAccountRepo::insert_account(&backend, &test_account).unwrap();

        let db_account = DefaultAccountRepo::get_account(backend).unwrap();
        assert_eq!(test_account, db_account);
    }
}
