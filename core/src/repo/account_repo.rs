use lockbook_models::account::Account;

use crate::model::errors::{core_err_unexpected, CoreError};
use crate::model::state::Config;
use crate::repo::local_storage;

static ACCOUNT: &str = "ACCOUNT";

#[instrument(level = "debug", skip(config, account), err(Debug))]
pub fn insert(config: &Config, account: &Account) -> Result<(), CoreError> {
    local_storage::write(
        config,
        ACCOUNT,
        ACCOUNT,
        bincode::serialize(account).map_err(core_err_unexpected)?,
    )
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(config: &Config) -> Result<Option<Account>, CoreError> {
    match get(config) {
        Ok(account) => Ok(Some(account)),
        Err(err) => match err {
            CoreError::AccountNonexistent => Ok(None),
            other => Err(other),
        },
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn get(config: &Config) -> Result<Account, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, ACCOUNT, ACCOUNT)?;
    match maybe_value {
        None => Err(CoreError::AccountNonexistent),
        Some(account) => Ok(bincode::deserialize(account.as_ref()).map_err(core_err_unexpected)?),
    }
}
