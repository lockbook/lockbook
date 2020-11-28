use std::io;

use sled::{Db, IVec};
use tempfile::tempdir;

use crate::model::state::Config;
use crate::DB_NAME;

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

#[derive(Debug)]
pub enum BackendError {
    SledError(sled::Error),
    NotWritten,
}

pub enum Backend<'a> {
    Sled(&'a Db),
}

impl Backend<'_> {
    pub fn write<K, V>(&self, key: K, value: V) -> Result<(), BackendError>
    where
        K: AsRef<[u8]>,
        V: Into<IVec>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db
                    .open_tree(b"documents")
                    .map_err(BackendError::SledError)?;
                tree.insert(key, value)
                    .map_err(BackendError::SledError)
                    .map(|_| ())
            }
        }
    }

    pub fn read<K, V>(&self, key: K) -> Result<Option<V>, BackendError>
    where
        K: AsRef<[u8]>,
        V: From<IVec>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db
                    .open_tree(b"documents")
                    .map_err(BackendError::SledError)?;
                tree.get(key)
                    .map_err(BackendError::SledError)
                    .map(|v| v.map(From::from))
            }
        }
    }

    pub fn delete<K>(&self, key: K) -> Result<(), BackendError>
    where
        K: AsRef<[u8]>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db
                    .open_tree(b"documents")
                    .map_err(BackendError::SledError)?;
                tree.remove(key)
                    .map_err(BackendError::SledError)
                    .map(|_| ())
            }
        }
    }
}
