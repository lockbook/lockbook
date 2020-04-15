use std::option::NoneError;

use serde_json;
use sled;
use sled::Db;

use crate::crypto::{KeyPair, PrivateKey, PublicKey};
use crate::error_enum;
use crate::model::account::Account;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        AccountMissing(NoneError),
    }
}

pub trait AccountRepo {
    fn insert_account(db: &Db, account: &Account) -> Result<(), Error>;
    fn get_account(db: &Db) -> Result<Account, Error>;
}

pub struct AccountRepoImpl;

impl AccountRepo for AccountRepoImpl {
    fn insert_account(db: &Db, account: &Account) -> Result<(), Error> {
        db.insert(b"0", serde_json::to_vec(account)?);
        Ok(())
    }

    fn get_account(db: &Db) -> Result<Account, Error> {
        let maybe_value = db.get(b"0")?;
        let val = maybe_value?;
        let account: Account = serde_json::from_slice(val.as_ref()).unwrap();
        Ok(account)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::crypto::{KeyPair, PrivateKey, PublicKey};
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, TempBackedDB};

    type DefaultDbProvider = TempBackedDB;
    type DefaultAcountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            keys: KeyPair {
                public_key: PublicKey {
                    n: "vec![1]".to_string(),
                    e: "vec![2]".to_string(),
                },
                private_key: PrivateKey {
                    d: "vec![3]".to_string(),
                    p: "vec![4]".to_string(),
                    q: "vec![5]".to_string(),
                    dmp1: "vec![6]".to_string(),
                    dmq1: "vec![7]".to_string(),
                    iqmp: "vec![8]".to_string(),
                },
            },
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let res = DefaultAcountRepo::get_account(&db);
        println!("{:?}", res);
        assert!(res.is_err());

        DefaultAcountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAcountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}
