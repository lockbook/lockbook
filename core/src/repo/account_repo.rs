use crate::model::state::Config;
use crate::repo::local_storage;
use crate::{core_err_unexpected, CoreError};
use lockbook_models::account::{Account, ApiUrl};

static ACCOUNT: &str = "account";
static YOU: &str = "you";

pub fn insert(config: &Config, account: &Account) -> Result<(), CoreError> {
    local_storage::write(
        config,
        ACCOUNT,
        YOU,
        serde_json::to_vec(account).map_err(core_err_unexpected)?,
    )
}

pub fn maybe_get(config: &Config) -> Result<Option<Account>, CoreError> {
    match get(config) {
        Ok(account) => Ok(Some(account)),
        Err(err) => match err {
            CoreError::AccountNonexistent => Ok(None),
            other => Err(other),
        },
    }
}

pub fn get(config: &Config) -> Result<Account, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, ACCOUNT, YOU)?;
    match maybe_value {
        None => Err(CoreError::AccountNonexistent),
        Some(account) => Ok(serde_json::from_slice(account.as_ref()).map_err(core_err_unexpected)?),
    }
}

pub fn get_api_url(config: &Config) -> Result<ApiUrl, CoreError> {
    get(config).map(|account| account.api_url)
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
        let res = account_repo::get(&config);
        assert!(res.is_err());

        account_repo::insert(&config, &test_account).unwrap();

        let db_account = account_repo::get(&config).unwrap();
        assert_eq!(test_account, db_account);
    }
}
