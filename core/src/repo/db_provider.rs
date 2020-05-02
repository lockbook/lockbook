use std::io;
use std::option::NoneError;

use crate::error_enum;
use crate::model::state::Config;
use crate::service::logging_service::Logger;
use crate::DB_NAME;
use sled::Db;
use std::marker::PhantomData;
use tempfile;
use tempfile::tempdir;

error_enum! {
    enum Error {
        SledError(sled::Error),
        TempFileError(io::Error),
        NoTempDir(NoneError),
    }
}

pub trait DbProvider {
    fn connect_to_db(config: &Config) -> Result<Db, Error>;
}

pub struct DiskBackedDB<Log: Logger> {
    log: PhantomData<Log>,
}

pub struct TempBackedDB;

impl<Log: Logger> DbProvider for DiskBackedDB<Log> {
    fn connect_to_db(config: &Config) -> Result<Db, Error> {
        let db_path = format!("{}/{}", &config.writeable_path, DB_NAME.to_string());
        Log::debug(format!("DB Location: {}", db_path));
        Ok(sled::open(db_path.as_str())?)
    }
}

impl DbProvider for TempBackedDB {
    fn connect_to_db(_config: &Config) -> Result<Db, Error> {
        let dir = tempdir()?;
        let dir_path = format!(
            "{}/{}",
            dir.path().to_str()?.to_string(),
            DB_NAME.to_string()
        );
        println!("{:?}", dir_path);
        Ok(sled::open(dir_path)?)
    }
}
