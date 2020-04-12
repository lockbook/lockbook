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
        // This gets called a lot maybe we'll not print it
        // println!("Connecting to DB at: {}", db_path);

        let db = Connection::open(db_path.as_str())?;

        match Schema::apply_schema(&db) {
            Ok(_) => {
                println!("Schema applied succesfully!");
                Ok(db)
            }
            Err(err) => {
                println!(
                    "Table creation failure, probably already exists, continuing! Error: {:?}",
                    err
                );
                Ok(db)
            }
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
