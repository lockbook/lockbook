use rusqlite::{Connection, params};

use crate::DB_NAME;
use crate::error_enum;
use crate::state::Config;

pub trait DbProvider {
    fn connect_to_db(config: Config) -> Result<Connection, Error>;
}

pub struct DbProviderImpl;

error_enum! {
    enum Error {
        ConnectionFailure(rusqlite::Error),
        TableCreationFailure(()),
    }
}

impl DbProvider for DbProviderImpl {
    fn connect_to_db(config: Config) -> Result<Connection, Error> {
        let db_path = config.writeable_path + "/" + DB_NAME;
        println!("Connecting to DB at: {}", db_path);

        let db = Connection::open(db_path.as_str())?;
        
        db.execute(
            "CREATE TABLE user_info (
                    username TEXT not null,
                    public_n TEXT not null,
                    public_e TEXT not null,
                    private_d TEXT not null,
                    private_p TEXT not null,
                    private_q TEXT not null,
                    private_dmp1 TEXT not null,
                    private_dmq1 TEXT not null,
                    private_iqmp TEXT not null,
                 )",
            params![],
        )?;

        Ok(db)
    }
}
