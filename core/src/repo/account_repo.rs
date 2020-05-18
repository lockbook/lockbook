use std::option::NoneError;

use serde_json;

use crate::error_enum;
use crate::model::account::Account;
use crate::repo::store::Store;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        AccountMissing(NoneError), // TODO: not required in get_account
    }
}

pub struct AccountRepo {
    pub store: Box<dyn Store>,
}

impl AccountRepo {
    pub fn insert_account(&self, account: &Account) -> Result<(), Error> {
        match &self
            .store
            .update(b"account".to_vec(), serde_json::to_vec(account)?)
        {
            Ok(_) => Ok(()),
            Err(_) => panic!("Unhandled error!"),
        }
    }

    pub fn get_account(&self) -> Result<Account, Error> {
        match &self.store.get(b"account".to_vec()) {
            Ok(val) => match val {
                Some(accBytes) => {
                    let account: Account = serde_json::from_slice(accBytes.as_slice())?;
                    Ok(account)
                }
                None => Err(Error::AccountMissing(NoneError)),
            },
            Err(_) => panic!("Unhandled error!"),
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
