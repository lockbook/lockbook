use std::ops::Try;
use std::option::NoneError;

use rusqlite::{Connection, params};

use crate::error_enum;
use crate::model::account::Account;

error_enum! {
    enum Error {
        DbError(rusqlite::Error),
        AccountRowMissing(NoneError),
        SerializationError(serde_json::error::Error),
    }
}

pub trait AccountRepo {
    fn insert_account(db: &Connection, account: &Account) -> Result<(), Error>;
    fn get_account(db: &Connection) -> Result<Account, Error>;
}

pub struct AccountRepoImpl;

impl AccountRepo for AccountRepoImpl {
    fn insert_account(db: &Connection, account: &Account) -> Result<(), Error> {
        db.execute(
            "INSERT INTO user_info (id, username, private_key) VALUES (0, ?1, ?2)",
            params![
                &account.username,
                serde_json::to_string(&account.keys)?
            ],
        )?;
        Ok(())
    }

    fn get_account(db: &Connection) -> Result<Account, Error> {
        let mut stmt = db.prepare(
            "SELECT username, private_key FROM user_info WHERE id = 0",
        )?;

        let mut user_iter = stmt.query_map(params![], |row| Ok(vec![row.get(0)?, row.get(1)?]))?;

        let maybe_row = user_iter.next().into_result()?;
        let maybe_vec:Vec<String> = maybe_row?;

        Ok(Account {
            username: maybe_vec.get(0).expect("Please get rid of sqlite").parse().unwrap(),
            keys: serde_json::from_str(maybe_vec[1].as_str())?,
        })
    }
}


#[cfg(test)]
mod unit_tests {
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, RamBackedDB};
    use crate::repo::schema::SchemaCreatorImpl;
    use crate::crypto::{RsaCryptoService, PubKeyCryptoService};

    type DefaultSchema = SchemaCreatorImpl;
    type DefaultDbProvider = RamBackedDB<DefaultSchema>;
    type DefaultAcountRepo = AccountRepoImpl;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            keys: RsaCryptoService::generate_key().expect("Key gen failed")
        };

        let config = &Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(config).unwrap();
        DefaultAcountRepo::insert_account(&db, &test_account).unwrap();

        let db_account = DefaultAcountRepo::get_account(&db).unwrap();
        assert_eq!(test_account, db_account);
    }
}

