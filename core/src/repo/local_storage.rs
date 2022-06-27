use std::fs::{self, create_dir_all, remove_file, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use crate::{Config, CoreError};

pub fn write<N, K, V>(db: &Config, namespace: N, key: K, value: V) -> Result<(), CoreError>
where
    N: AsRef<[u8]>,
    K: AsRef<[u8]>,
    V: Into<Vec<u8>>,
{
    let path_str = key_path(db, &namespace, &key) + ".pending";
    let path = Path::new(&path_str);
    let data = &value.into();
    trace!("write\t{} {:?} bytes", &path_str, data.len());
    create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    f.write_all(data).map_err(CoreError::from)?;
    fs::rename(path, key_path(db, &namespace, &key)).map_err(CoreError::from)
}

pub fn read<N, K, V>(db: &Config, namespace: N, key: K) -> Result<Option<V>, CoreError>
where
    N: AsRef<[u8]>,
    K: AsRef<[u8]>,
    V: From<Vec<u8>>,
{
    let path_str = key_path(db, namespace, key);
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
            _ => Err(err.into()),
        },
    }
}

pub fn delete<N, K>(db: &Config, namespace: N, key: K) -> Result<(), CoreError>
where
    N: AsRef<[u8]>,
    K: AsRef<[u8]>,
{
    let path_str = key_path(db, namespace, key);
    let path = Path::new(&path_str);
    trace!("delete\t{}", &path_str);
    if path.exists() {
        remove_file(path).map_err(CoreError::from)
    } else {
        Ok(())
    }
}

pub fn delete_all<N>(db: &Config, namespace: N) -> Result<(), CoreError>
where
    N: AsRef<[u8]>,
{
    let path_str = namespace_path(db, namespace);
    trace!("delete_all\t{}", path_str);
    // note: this fails if a file is deleted between call to read_dir and subsequent calls to remove_file
    if let Ok(rd) = fs::read_dir(path_str) {
        for entry in rd {
            fs::remove_file(entry?.path())?;
        }
    }

    Ok(())
}

pub fn namespace_path<N>(db: &Config, namespace: N) -> String
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
    format!("{}/{}", namespace_path(db, namespace), k)
}
