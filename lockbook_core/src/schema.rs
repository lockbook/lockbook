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
                    public_n TEXT not null,
                    public_e TEXT not null,
                    private_d TEXT not null,
                    private_p TEXT not null,
                    private_q TEXT not null,
                    private_dmp1 TEXT not null,
                    private_dmq1 TEXT not null,
                    private_iqmp TEXT not null
                 );",
            params![],
        )?;

        Ok(())
    }
}
