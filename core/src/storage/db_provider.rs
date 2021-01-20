use crate::model::state::Config;
use crate::DB_NAME;
use std::fmt::Debug;
use std::fs::{create_dir_all, read_dir, remove_file, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

// Debug required because we parametrize enums with this, even though we only include Backend::Error
// https://github.com/rust-lang/rust/issues/26925
pub trait Backend: Debug {
    type Db;
    type Error: Debug;

    fn connect_to_db(config: &Config) -> Result<Self::Db, Self::Error>;
    fn write<N, K, V>(db: &Self::Db, namespace: N, key: K, value: V) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: Into<Vec<u8>>;
    fn read<N, K, V>(db: &Self::Db, namespace: N, key: K) -> Result<Option<V>, Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: From<Vec<u8>>;
    fn delete<N, K>(db: &Self::Db, namespace: N, key: K) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>;
    fn dump<N, V>(db: &Self::Db, namespace: N) -> Result<Vec<V>, Self::Error>
    where
        N: AsRef<[u8]> + Copy,
        V: From<Vec<u8>>;
}

#[derive(Debug)]
pub struct SledBackend;

impl Backend for SledBackend {
    type Db = sled::Db;
    type Error = sled::Error;

    fn connect_to_db(config: &Config) -> Result<Self::Db, Self::Error> {
        let db_path = format!("{}/{}", &config.writeable_path, DB_NAME.to_string());
        debug!("DB Location: {}", db_path);
        Ok(sled::open(db_path.as_str())?)
    }

    fn write<N, K, V>(db: &Self::Db, namespace: N, key: K, value: V) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: Into<Vec<u8>>,
    {
        db.open_tree(namespace)?
            .insert(key, value.into())
            .map(|_| ())
    }

    fn read<N, K, V>(db: &Self::Db, namespace: N, key: K) -> Result<Option<V>, Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: From<Vec<u8>>,
    {
        db.open_tree(namespace)?
            .get(key)
            .map(|v| v.map(|d| From::from(d.to_vec())))
    }

    fn delete<N, K>(db: &Self::Db, namespace: N, key: K) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
    {
        db.open_tree(namespace)?.remove(key).map(|_| ())
    }

    fn dump<N, V>(db: &Self::Db, namespace: N) -> Result<Vec<V>, Self::Error>
    where
        N: AsRef<[u8]> + Copy,
        V: From<Vec<u8>>,
    {
        db.open_tree(namespace)?
            .iter()
            .map(|s| s.map(|v| From::from(v.1.to_vec())))
            .collect()
    }
}

#[derive(Debug)]
pub struct FileBackend;

impl Backend for FileBackend {
    type Db = Config;
    type Error = std::io::Error;

    fn connect_to_db(config: &Config) -> Result<Self::Db, Self::Error> {
        Ok(config.clone())
    }

    fn write<N, K, V>(db: &Self::Db, namespace: N, key: K, value: V) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: Into<Vec<u8>>,
    {
        let path_str = Self::key_path(db, namespace, key);
        let path = Path::new(&path_str);
        let data = &value.into();
        trace!("write\t{} {:?} bytes", &path_str, data.len());
        create_dir_all(path.parent().unwrap())?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        f.write_all(data)
    }

    fn read<N, K, V>(db: &Self::Db, namespace: N, key: K) -> Result<Option<V>, Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
        V: From<Vec<u8>>,
    {
        let path_str = Self::key_path(db, namespace, key);
        let path = Path::new(&path_str);
        trace!("read\t{}", &path_str);
        match File::open(path) {
            Ok(mut f) => {
                let mut buffer: Vec<u8> = Vec::new();
                f.read_to_end(&mut buffer)?;
                Ok(Some(From::from(buffer)))
            }
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err),
            },
        }
    }

    fn delete<N, K>(db: &Self::Db, namespace: N, key: K) -> Result<(), Self::Error>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
    {
        let path_str = Self::key_path(db, namespace, key);
        let path = Path::new(&path_str);
        trace!("delete\t{}", &path_str);
        if path.exists() {
            remove_file(path)
        } else {
            Ok(())
        }
    }

    fn dump<N, V>(db: &Self::Db, namespace: N) -> Result<Vec<V>, Self::Error>
    where
        N: AsRef<[u8]> + Copy,
        V: From<Vec<u8>>,
    {
        let path_str = Self::namespace_path(db, namespace);
        let path = Path::new(&path_str);
        trace!("dump\t{}", &path_str);
        match read_dir(path) {
            Ok(rd) => rd
                .map(|e| {
                    e.and_then(|de| {
                        Ok(
                            Self::read(db, namespace, de.file_name().into_string().unwrap())
                                .map(|r| r.unwrap())?,
                        )
                    })
                })
                .collect::<Result<Vec<V>, Self::Error>>(),
            Err(_) => Ok(Vec::new()),
        }
    }
}

impl FileBackend {
    fn namespace_path<N>(db: &Config, namespace: N) -> String
    where
        N: AsRef<[u8]>,
    {
        let n = String::from_utf8_lossy(namespace.as_ref()).to_string();
        format!("{}/{}", db.writeable_path, n)
    }

    fn key_path<N, K>(db: &Config, namespace: N, key: K) -> String
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
    {
        let k = String::from_utf8_lossy(key.as_ref()).to_string();
        format!("{}/{}", Self::namespace_path(db, namespace), k)
    }
}
