use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoError::NoAccount;
use crate::storage::db_provider::FileBackend;
use lockbook_models::account::{Account, ApiUrl};

#[derive(Debug)]
pub enum AccountRepoError {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
    NoAccount,
}

pub trait AccountRepo {
    fn insert_account(config: &Config, account: &Account) -> Result<(), AccountRepoError>;
    fn maybe_get_account(config: &Config) -> Result<Option<Account>, AccountRepoError>;
    fn get_account(config: &Config) -> Result<Account, AccountRepoError>;
    fn get_api_url(config: &Config) -> Result<ApiUrl, AccountRepoError>;
}

pub struct AccountRepoImpl;

static ACCOUNT: &str = "account";
static YOU: &str = "you";

impl AccountRepo for AccountRepoImpl {
    fn insert_account(config: &Config, account: &Account) -> Result<(), AccountRepoError> {
        FileBackend::write(
            config,
            ACCOUNT,
            YOU,
            serde_json::to_vec(account).map_err(AccountRepoError::SerdeError)?,
        )
        .map_err(AccountRepoError::BackendError)
    }

    fn maybe_get_account(config: &Config) -> Result<Option<Account>, AccountRepoError> {
        match Self::get_account(config) {
            Ok(account) => Ok(Some(account)),
            Err(err) => match err {
                AccountRepoError::NoAccount => Ok(None),
                other => Err(other),
            },
        }
    }

    fn get_account(config: &Config) -> Result<Account, AccountRepoError> {
        let maybe_value: Option<Vec<u8>> =
            FileBackend::read(config, ACCOUNT, YOU).map_err(AccountRepoError::BackendError)?;
        match maybe_value {
            None => Err(NoAccount),
            Some(account) => {
                Ok(serde_json::from_slice(account.as_ref())
                    .map_err(AccountRepoError::SerdeError)?)
            }
        }
    }

    fn get_api_url(config: &Config) -> Result<ApiUrl, AccountRepoError> {
        Self::get_account(config).map(|account| account.api_url)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::AccountRepoImpl;
    use crate::model::state::temp_config;
    use crate::repo::account_repo::AccountRepo;
    use crate::storage::db_provider::{Backend, FileBackend};
    use lockbook_crypto::clock_service::ClockImpl;
    use lockbook_crypto::crypto_service::{PubKeyCryptoService, RSAImpl};
    use lockbook_models::account::Account;

    type DefaultAccountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            api_url: "ftp://uranus.net".to_string(),
            private_key: RSAImpl::<ClockImpl>::generate_key().expect("Key generation failure"),
        };

        let config = temp_config();
        let config = FileBackend::connect_to_db(&config).unwrap();
        let res = DefaultAccountRepo::get_account(&config);
        assert!(res.is_err());

        DefaultAccountRepo::insert_account(&config, &test_account).unwrap();

        let db_account = DefaultAccountRepo::get_account(&config).unwrap();
        assert_eq!(test_account, db_account);
    }
}
