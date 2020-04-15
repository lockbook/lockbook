use rusqlite::params;
use rusqlite::Connection;

use crate::error_enum;

error_enum! {
    enum Error {
        TableCreationFailure(rusqlite::Error),
    }
}

pub trait SchemaApplier {
    fn apply_schema(db: &Connection) -> Result<(), Error>;
}

pub struct SchemaCreatorImpl;

impl SchemaApplier for SchemaCreatorImpl {
    fn apply_schema(db: &Connection) -> Result<(), Error> {
        db.execute(
            "CREATE TABLE user_info (
                    id INTEGER PRIMARY KEY,
                    username TEXT not null,
                    private_key TEXT not null,
                 );",
            params![],
        )?;

        db.execute(
            "CREATE TABLE file_metadata (
                    id TEXT PRIMARY KEY,
                    name TEXT not null,
                    path TEXT not null,
                    updated_at INTEGER not null,
                    status TEXT not null
                 );",
            params![],
        )?;

        Ok(())
    }
}
