use sled::Db;

use crate::error_enum;
use crate::model::account::Account;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        AccountMissing(()),
    }
}

pub trait AccountRepo {
    fn insert_account(db: &Db, account: &Account) -> Result<(), Error>;
    fn get_account(db: &Db) -> Result<Account, Error>;
}

pub struct AccountRepoImpl;

static ACCOUNT: &str = "account";

impl AccountRepo for AccountRepoImpl {
    fn insert_account(db: &Db, account: &Account) -> Result<(), Error> {
        let tree = db.open_tree(ACCOUNT)?;
        tree.insert("you", serde_json::to_vec(account)?)?;
        Ok(())
    }

    fn get_account(db: &Db) -> Result<Account, Error> {
        let tree = db.open_tree(ACCOUNT)?;
        let maybe_value = tree.get("you")?;
        let val = maybe_value.ok_or(())?;
        let account: Account = serde_json::from_slice(val.as_ref())?;
        Ok(account)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::service::crypto_service::{PubKeyCryptoService, RsaImpl};

    type DefaultDbProvider = TempBackedDB;
    type DefaultAccountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            keys: RsaImpl::generate_key().expect("Key generation failure"),
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let res = DefaultAccountRepo::get_account(&db);
        assert!(res.is_err());

        DefaultAccountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAccountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}
