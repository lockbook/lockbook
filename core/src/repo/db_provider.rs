use std::io;

use sled::Db;
use tempfile::tempdir;

use crate::DB_NAME;
use lockbook_models::state::Config;

#[derive(Debug)]
pub enum Error {
    SledError(sled::Error),
    TempFileError(io::Error), // TODO ungroup these
}

pub trait DbProvider {
    fn connect_to_db(config: &Config) -> Result<Db, Error>;
}

pub struct DiskBackedDB;

pub struct TempBackedDB;

impl DbProvider for DiskBackedDB {
    fn connect_to_db(config: &Config) -> Result<Db, Error> {
        let db_path = format!("{}/{}", &config.writeable_path, DB_NAME.to_string());
        debug!("DB Location: {}", db_path);
        Ok(sled::open(db_path.as_str()).map_err(Error::SledError)?)
    }
}

impl DbProvider for TempBackedDB {
    fn connect_to_db(_config: &Config) -> Result<Db, Error> {
        let dir = tempdir().map_err(Error::TempFileError)?;
        let dir_path = dir.path().join(DB_NAME);
        debug!("DB Location: {:?}", dir_path);
        Ok(sled::open(dir_path).map_err(Error::SledError)?)
    }
}
