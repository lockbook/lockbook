use std::marker::PhantomData;

use rusqlite::Connection;

use crate::db_provider;
use crate::db_provider::DbProvider;
use crate::state::{Config, Account};
use crate::account_repo::Error::ConnectionError;

pub trait AccountRepo {
    fn insert_account(config: Config) -> Result<Account, Error>;
}

pub struct AccountRepoImpl<DB: DbProvider> {
    db: PhantomData<DB>,
}

pub enum Error {
    ConnectionError(db_provider::Error)
}

impl<DB: DbProvider> AccountRepo for AccountRepoImpl<DB> {
    fn insert_account(config: Config) -> Result<Account, Error> {
        match DB::connect_to_db(config) {
            Err(err) => Err(ConnectionError(err)),
            Ok(db) => Ok(db),
        }.and_then(|db| db.execute() )
    }
}
