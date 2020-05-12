use std::env;

use lockbook_core::model::state::Config;

use lockbook_core::repo::db_provider::DbProvider;

use lockbook_core::model::account::Account;
use lockbook_core::repo::account_repo::{AccountRepo, Error};
use lockbook_core::{Db, DefaultAccountRepo, DefaultDbProvider};

pub fn connect_to_db() -> Db {
    // Save data in LOCKBOOK_CLI_LOCATION or ~/.lockbook/
    let path = env::var("LOCKBOOK_CLI_LOCATION")
        .unwrap_or(format!("{}/.lockbook", env::var("HOME")
            .expect("Could not read env var LOCKBOOK_CLI_LOCATION or HOME, don't know where to place your .lockbook folder"))
        );

    DefaultDbProvider::connect_to_db(&Config {
        writeable_path: path.clone(),
    })
    .expect(&format!("Could not connect to db at path: {}", path))
}

pub fn get_account(db: &Db) -> Account {
    // DefaultAccountRepo::get_account(&db).expect("test")
    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => account,
        Err(err) => match err {
            Error::SledError(err) => {
                panic!("No account found, run init, import or help. Error: {}", err)
            }
            Error::SerdeError(err) => panic!("Account data corrupted: {}", err),
            Error::AccountMissing(err) => panic!("No account found, run init, import or help. Error: {:?}", err),
        },
    }
}

pub fn get_editor() -> String {
    env::var("VISUAL").unwrap_or(env::var("EDITOR").unwrap_or("vi".to_string()))
}
