use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoError::NoAccount;
use crate::repo::local_storage;
use lockbook_models::account::{Account, ApiUrl};

#[derive(Debug)]
pub enum AccountRepoError {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
    NoAccount,
}

static ACCOUNT: &str = "account";
static YOU: &str = "you";

pub fn insert_account(config: &Config, account: &Account) -> Result<(), AccountRepoError> {
    local_storage::write(
        config,
        ACCOUNT,
        YOU,
        serde_json::to_vec(account).map_err(AccountRepoError::SerdeError)?,
    )
    .map_err(AccountRepoError::BackendError)
}

pub fn maybe_get_account(config: &Config) -> Result<Option<Account>, AccountRepoError> {
    match get_account(config) {
        Ok(account) => Ok(Some(account)),
        Err(err) => match err {
            AccountRepoError::NoAccount => Ok(None),
            other => Err(other),
        },
    }
}

pub fn get_account(config: &Config) -> Result<Account, AccountRepoError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, ACCOUNT, YOU).map_err(AccountRepoError::BackendError)?;
    match maybe_value {
        None => Err(NoAccount),
        Some(account) => {
            Ok(serde_json::from_slice(account.as_ref()).map_err(AccountRepoError::SerdeError)?)
        }
    }
}

pub fn get_api_url(config: &Config) -> Result<ApiUrl, AccountRepoError> {
    get_account(config).map(|account| account.api_url)
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;

    use crate::repo::account_repo;
    use lockbook_crypto::pubkey;
    use lockbook_models::account::Account;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            api_url: "ftp://uranus.net".to_string(),
            private_key: pubkey::generate_key(),
        };

        let config = temp_config();
        let res = account_repo::get_account(&config);
        assert!(res.is_err());

        account_repo::insert_account(&config, &test_account).unwrap();

        let db_account = account_repo::get_account(&config).unwrap();
        assert_eq!(test_account, db_account);
    }
}
