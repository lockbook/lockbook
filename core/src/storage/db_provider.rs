use sled::Db;

use crate::model::state::Config;
use crate::DB_NAME;
use std::fs::{create_dir_all, read_dir, remove_file, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

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
    FileError(std::io::Error),
    NotWritten,
}

pub enum Backend<'a> {
    Sled(&'a Db),
    File(&'a Config),
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
            Backend::File(config) => {
                let n = String::from_utf8_lossy(namespace.as_ref()).to_string();
                let k = String::from_utf8_lossy(key.as_ref()).to_string();
                let path_str = format!("{}/{}/{}", config.writeable_path, n, k);
                let path = Path::new(&path_str);
                let data = &value.into();
                trace!("write\t{} {:?} bytes", &path_str, data.len());
                create_dir_all(path.parent().unwrap()).map_err(BackendError::FileError)?;
                let mut f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)
                    .map_err(BackendError::FileError)?;
                f.write_all(data).map_err(BackendError::FileError)
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
            Backend::File(config) => {
                let n = String::from_utf8_lossy(namespace.as_ref()).to_string();
                let k = String::from_utf8_lossy(key.as_ref()).to_string();
                let path_str = format!("{}/{}/{}", config.writeable_path, n, k);
                let path = Path::new(&path_str);
                trace!("read\t{}", &path_str);
                match File::open(path) {
                    Ok(mut f) => {
                        let mut buffer: Vec<u8> = Vec::new();
                        f.read_to_end(&mut buffer)
                            .map_err(BackendError::FileError)?;
                        Ok(Some(From::from(buffer)))
                    }
                    Err(err) => match err.kind() {
                        ErrorKind::NotFound => Ok(None),
                        _ => Err(BackendError::FileError(err)),
                    },
                }
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
            Backend::File(config) => {
                let n = String::from_utf8_lossy(namespace.as_ref()).to_string();
                let k = String::from_utf8_lossy(key.as_ref()).to_string();
                let path_str = format!("{}/{}/{}", config.writeable_path, n, k);
                let path = Path::new(&path_str);
                trace!("delete\t{}", &path_str);
                remove_file(path).map_err(BackendError::FileError)
            }
        }
    }

    pub fn dump<N, V>(&self, namespace: N) -> Result<Vec<V>, BackendError>
    where
        N: AsRef<[u8]>,
        V: From<Vec<u8>>,
    {
        match self {
            Backend::Sled(db) => {
                let tree = db.open_tree(namespace).map_err(BackendError::SledError)?;
                tree.iter()
                    .map(|s| {
                        s.map(|v| From::from(v.1.to_vec()))
                            .map_err(BackendError::SledError)
                    })
                    .collect()
            }
            Backend::File(config) => {
                let n = String::from_utf8_lossy(&namespace.as_ref()).to_string();
                let path_str = format!("{}/{}", config.writeable_path, n);
                let path = Path::new(&path_str);
                trace!("dump\t{}", &path_str);
                match read_dir(path) {
                    Ok(rd) => rd
                        .map(|e| {
                            e.map_err(BackendError::FileError).and_then(|de| {
                                self.read(&namespace, de.file_name().into_string().unwrap())
                                    .map(|r| r.unwrap())
                            })
                        })
                        .collect::<Result<Vec<V>, BackendError>>(),
                    Err(_) => Ok(Vec::new()),
                }
            }
        }
    }
}
