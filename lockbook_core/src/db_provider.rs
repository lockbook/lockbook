use rusqlite::{Connection, params};

use crate::DB_NAME;
use crate::db_provider::Error::{ConnectionFailure, TableCreationFailure};
use crate::state::Config;

pub trait DbProvider {
    fn connect_to_db(config: Config) -> Result<Connection, Error>;
}

pub struct DbProviderImpl;

pub enum Error {
    ConnectionFailure(rusqlite::Error),
    TableCreationFailure,
}

impl DbProvider for DbProviderImpl {
    fn connect_to_db(config: Config) -> Result<Connection, Error> {
        let db_path = config.writeable_path + "/" + DB_NAME;
        println!("Connecting to DB at: {}", db_path);

        match Connection::open(db_path.as_str()) {
            Ok(db) => Ok(db),
            Err(err) => {
                eprintln!("Failed to connect to DB: {}", err);
                Err(ConnectionFailure(err))
            }
        }.and_then(|db|

            // TODO figure out what properties go in here, how to get the subkeys out of the private key
            match db.execute(
                "CREATE TABLE user_info (
                    username TEXT,
                    d TEXT,
                    p TEXT,
                    p TEXT,
                   )",
                params![]) {
                Ok(_) => Ok(db),
                Err(_) => Err(TableCreationFailure),
            }
        )
    }
}