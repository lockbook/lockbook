use std::option::NoneError;

use serde_json;
use sled;
use sled::Db;

use crate::error_enum;
use crate::model::account::Account;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        AccountMissing(NoneError), // TODO: not required in get_account
    }
}

pub trait AccountRepo {
    fn insert_account(db: &Db, account: &Account) -> Result<(), Error>;
    fn get_account(db: &Db) -> Result<Account, Error>;
}

pub struct AccountRepoImpl;

impl AccountRepo for AccountRepoImpl {
    fn insert_account(_db: &Db, account: &Account) -> Result<(), Error> {
        // let tree = db.open_tree("account")?;
        // tree.insert("you", serde_json::to_vec(account)?)?;
        // documents
        let path = std::path::Path::new(crate::JUNK);
        match std::fs::write(path, serde_json::to_vec(account)?) {
            Ok(_) => {
                debug!("Wrote some new shit to junk");
                Ok(())
            }
            Err(err) => {
                panic!("Failed to write to junk! {:?}", err);
            }
        }
    }

    fn get_account(_db: &Db) -> Result<Account, Error> {
        // let tree = db.open_tree("account")?;
        // let maybe_value = tree.get("you")?;
        // let val = maybe_value?;
        // let account: Account = serde_json::from_slice(val.as_ref())?;
        let path = std::path::Path::new(crate::JUNK);
        match std::fs::read(path) {
            Ok(val) => {
                debug!("Junk Contents: {:?}", String::from_utf8(val.clone()));
                let account: Account = serde_json::from_slice(val.as_ref())?;
                Ok(account)
            }
            Err(err) => panic!("Failed to read junk! {:?}", err),
        }
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
