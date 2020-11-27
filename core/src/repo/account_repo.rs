use sled::Db;

use crate::repo::account_repo::AccountRepoError::NoAccount;
use crate::repo::account_repo::DbError::{SerdeError, SledError};
use lockbook_models::account::{Account, ApiUrl};

#[derive(Debug)]
pub enum DbError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

#[derive(Debug)]
pub enum AccountRepoError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
    NoAccount,
}

pub trait AccountRepo {
    fn insert_account(db: &Db, account: &Account) -> Result<(), AccountRepoError>;
    fn maybe_get_account(db: &Db) -> Result<Option<Account>, DbError>;
    fn get_account(db: &Db) -> Result<Account, AccountRepoError>;
    fn get_api_url(db: &Db) -> Result<ApiUrl, AccountRepoError>;
}

pub struct AccountRepoImpl;

static ACCOUNT: &str = "account";
static YOU: &str = "you";

impl AccountRepo for AccountRepoImpl {
    fn insert_account(db: &Db, account: &Account) -> Result<(), AccountRepoError> {
        let tree = db.open_tree(ACCOUNT).map_err(AccountRepoError::SledError)?;
        tree.insert(
            YOU,
            serde_json::to_vec(account).map_err(AccountRepoError::SerdeError)?,
        )
        .map_err(AccountRepoError::SledError)?;
        Ok(())
    }

    fn maybe_get_account(db: &Db) -> Result<Option<Account>, DbError> {
        match Self::get_account(&db) {
            Ok(account) => Ok(Some(account)),
            Err(err) => match err {
                AccountRepoError::NoAccount => Ok(None),
                AccountRepoError::SledError(sled) => Err(SledError(sled)),
                AccountRepoError::SerdeError(serde) => Err(SerdeError(serde)),
            },
        }
    }

    fn get_account(db: &Db) -> Result<Account, AccountRepoError> {
        let tree = db.open_tree(ACCOUNT).map_err(AccountRepoError::SledError)?;
        let maybe_value = tree.get(YOU).map_err(AccountRepoError::SledError)?;
        match maybe_value {
            None => Err(NoAccount),
            Some(account) => {
                Ok(serde_json::from_slice(account.as_ref())
                    .map_err(AccountRepoError::SerdeError)?)
            }
        }
    }

    fn get_api_url(db: &Db) -> Result<ApiUrl, AccountRepoError> {
        let tree = db.open_tree(ACCOUNT).map_err(AccountRepoError::SledError)?;
        tree.get(YOU)
            .map_err(AccountRepoError::SledError)?
            .ok_or(AccountRepoError::NoAccount)
            .and_then(|data| {
                serde_json::from_slice(data.as_ref()).map_err(AccountRepoError::SerdeError)
            })
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::service::crypto_service::{PubKeyCryptoService, RsaImpl};
    use lockbook_models::account::Account;
    use lockbook_models::state::dummy_config;

    type DefaultDbProvider = TempBackedDB;
    type DefaultAccountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            api_url: "ftp://uranus.net".to_string(),
            keys: RsaImpl::generate_key().expect("Key generation failure"),
        };

        let config = dummy_config();
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let res = DefaultAccountRepo::get_account(&db);
        assert!(res.is_err());

        DefaultAccountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAccountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}
