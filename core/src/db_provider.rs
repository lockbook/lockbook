use std::marker::PhantomData;

use rusqlite::Connection;

use crate::error_enum;
use crate::schema;
use crate::schema::SchemaApplier;
use crate::state::Config;
use crate::DB_NAME;

error_enum! {
    enum Error {
        ConnectionFailure(rusqlite::Error),
        SchemaError(schema::Error),
    }
}

pub trait DbProvider {
    fn connect_to_db(config: &Config) -> Result<Connection, Error>;
}

pub struct DiskBackedDB<Schema: SchemaApplier> {
    schema: PhantomData<Schema>,
}

pub struct RamBackedDB<Schema: SchemaApplier> {
    schema: PhantomData<Schema>,
}

impl<Schema: SchemaApplier> DbProvider for DiskBackedDB<Schema> {
    fn connect_to_db(config: &Config) -> Result<Connection, Error> {
        let db_path = format!("{}/{}", &config.writeable_path, DB_NAME.to_string());
        let db = Connection::open(db_path.as_str())?;

        match Schema::apply_schema(&db) {
            Ok(_) => {
                println!("💚 Schema applied succesfully!");
                Ok(db)
            }
            // TODO: This should be handled better or a new library
            Err(err) => match err {
                schema::Error::TableCreationFailure(rusqlite::Error::SqliteFailure(
                    sqlite_err,
                    Some(msg),
                )) => {
                    if msg.contains("already exists") {
                        println!("💚 Table already exists! {}", msg);
                        Ok(db)
                    } else {
                        return Err(Error::SchemaError(schema::Error::TableCreationFailure(
                            rusqlite::Error::SqliteFailure(sqlite_err, Some(msg)),
                        )));
                    }
                }
                _ => Err(Error::SchemaError(err)),
            },
        }
    }
}

impl<Schema: SchemaApplier> DbProvider for RamBackedDB<Schema> {
    fn connect_to_db(_config: &Config) -> Result<Connection, Error> {
        let db = Connection::open_in_memory()?;
        Schema::apply_schema(&db)?;

        Ok(db)
    }
}
