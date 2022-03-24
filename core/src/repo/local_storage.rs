use std::fs::{self, create_dir_all, read_dir, remove_file, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use crate::model::errors::core_err_unexpected;
use crate::model::state::Config;
use crate::CoreError;

pub fn write<N, K, V>(db: &Config, namespace: N, key: K, value: V) -> Result<(), CoreError>
where
    N: AsRef<[u8]>,
    K: AsRef<[u8]>,
    V: Into<Vec<u8>>,
{
    let path_str = key_path(db, namespace, key);
    let path = Path::new(&path_str);
    let data = &value.into();
    trace!("write\t{} {:?} bytes", &path_str, data.len());
    create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    f.write_all(data).map_err(CoreError::from)
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

pub fn dump<N, V>(db: &Config, namespace: N) -> Result<Vec<V>, CoreError>
where
    N: AsRef<[u8]> + Copy,
    V: From<Vec<u8>>,
{
    let path_str = namespace_path(db, namespace);
    let path = Path::new(&path_str);

    match read_dir(path) {
        Ok(rd) => {
            let mut file_names = rd
                .map(|dir_entry| {
                    dir_entry
                        .map_err(CoreError::from)?
                        .file_name()
                        .into_string()
                        .map_err(core_err_unexpected)
                })
                .collect::<Result<Vec<String>, CoreError>>()?;
            file_names.sort();

            file_names
                .iter()
                .map(|file_name| {
                    read(db, namespace, file_name)?.ok_or_else(|| {
                        CoreError::Unexpected(String::from(
                            "file listed in directory was not found when we tried to read it",
                        ))
                    })
                })
                .collect::<Result<Vec<V>, CoreError>>()
        }
        Err(_) => Ok(Vec::new()),
    }
}

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
    format!("{}/{}", namespace_path(db, namespace), k)
}

#[cfg(test)]
mod unit_tests {
    use crate::model::state::temp_config;
    use crate::repo::local_storage;

    #[test]
    fn read() {
        let db = &temp_config();

        let result: Option<Vec<u8>> = local_storage::read(db, "namespace", "key").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn write_read() {
        let db = &temp_config();

        local_storage::write(db, "namespace", "key", "value".as_bytes()).unwrap();
        let result: Vec<u8> = local_storage::read(db, "namespace", "key")
            .unwrap()
            .unwrap();

        assert_eq!(String::from_utf8_lossy(&result), "value");
    }

    #[test]
    fn overwrite_read() {
        let db = &temp_config();

        local_storage::write(db, "namespace", "key", "value-1".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key", "value-2".as_bytes()).unwrap();
        let result: Vec<u8> = local_storage::read(db, "namespace", "key")
            .unwrap()
            .unwrap();

        assert_eq!(String::from_utf8_lossy(&result), "value-2");
    }

    #[test]
    fn delete() {
        let db = &temp_config();

        local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
        local_storage::delete(db, "namespace", "key-2").unwrap();
        let result1: Vec<u8> = local_storage::read(db, "namespace", "key-1")
            .unwrap()
            .unwrap();
        let result2: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-2").unwrap();

        assert_eq!(String::from_utf8_lossy(&result1), "value-1");
        assert_eq!(result2, None);
    }

    #[test]
    fn delete_all() {
        let db = &temp_config();

        local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
        local_storage::delete_all(db, "namespace").unwrap();
        let result1: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-1").unwrap();
        let result2: Option<Vec<u8>> = local_storage::read(db, "namespace", "key-2").unwrap();

        assert_eq!(result1, None);
        assert_eq!(result2, None);
    }

    #[test]
    fn delete_all_no_writes() {
        let db = &temp_config();

        local_storage::delete_all(db, "namespace").unwrap();
    }

    #[test]
    fn dump() {
        let db = &temp_config();

        local_storage::write(db, "namespace", "key-1", "value-1".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-4", "value-4".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-3", "value-3".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-2", "value-2".as_bytes()).unwrap();
        local_storage::write(db, "namespace", "key-5", "value-5".as_bytes()).unwrap();

        let result: Vec<Vec<u8>> = local_storage::dump(db, "namespace").unwrap();

        assert_eq!(
            result,
            vec![
                "value-1".as_bytes(),
                "value-2".as_bytes(),
                "value-3".as_bytes(),
                "value-4".as_bytes(),
                "value-5".as_bytes(),
            ]
        );
    }
}
