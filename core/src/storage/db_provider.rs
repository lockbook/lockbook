use sled::Db;

use crate::model::state::Config;
use crate::DB_NAME;

#[derive(Debug)]
pub enum Error {
    SledError(sled::Error),
}

pub trait DbProvider {
    fn connect_to_db(config: &Config) -> Result<Db, Error>;
}

pub fn to_backend(db: &Db) -> Backend {
    Backend::Sled(db)
}

pub struct DiskBackedDB;

impl DbProvider for DiskBackedDB {
    fn connect_to_db(config: &Config) -> Result<Db, Error> {
        let db_path = format!("{}/{}", &config.writeable_path, DB_NAME.to_string());
        debug!("DB Location: {}", db_path);
        Ok(sled::open(db_path.as_str()).map_err(Error::SledError)?)
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
    pub fn write<N, K, V>(&self, namespace: N, key: K, value: V) -> Result<(), BackendError>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: Into<Vec<u8>>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db.open_tree(namespace).map_err(BackendError::SledError)?;
                tree.insert(key, value.into())
                    .map_err(BackendError::SledError)
                    .map(|_| ())
            }
        }
    }

    pub fn read<N, K, V>(&self, namespace: N, key: K) -> Result<Option<V>, BackendError>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: From<Vec<u8>>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db.open_tree(namespace).map_err(BackendError::SledError)?;
                tree.get(key)
                    .map_err(BackendError::SledError)
                    .map(|v| v.map(|d| From::from(d.to_vec())))
            }
        }
    }

    pub fn delete<N, K>(&self, namespace: N, key: K) -> Result<(), BackendError>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db.open_tree(namespace).map_err(BackendError::SledError)?;
                tree.remove(key)
                    .map_err(BackendError::SledError)
                    .map(|_| ())
            }
        }
    }
}
