extern crate tracing;

use crate::core_config::Config;
use crate::crypto::*;
use crate::{SharedError, SharedResult};
use tracing::*;
use uuid::Uuid;

const NAMESPACE_LOCAL: &str = "changed_local_documents";
const NAMESPACE_BASE: &str = "all_base_documents";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RepoSource {
    Local, // files with local edits applied
    Base,  // files at latest known state when client and server matched
}

pub fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

#[instrument(level = "debug", skip(config, document), err(Debug))]
pub fn insert(
    config: &Config, source: RepoSource, id: Uuid, document: &EncryptedDocument,
) -> SharedResult<()> {
    local_storage::write(
        config,
        namespace(source),
        id.to_string().as_str(),
        bincode::serialize(document)?,
    )
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn get(config: &Config, source: RepoSource, id: Uuid) -> SharedResult<EncryptedDocument> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Err(SharedError::FileNonexistent),
        Some(data) => Ok(bincode::deserialize(&data)?),
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(
    config: &Config, source: RepoSource, id: &Uuid,
) -> SharedResult<Option<EncryptedDocument>> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Ok(None),
        Some(data) => Ok(bincode::deserialize(&data).map(Some)?),
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> SharedResult<()> {
    local_storage::delete(config, namespace(source), id.to_string().as_str())
}

pub mod local_storage {
    use crate::core_config::Config;
    use crate::SharedResult;
    use std::fs::{self, create_dir_all, remove_file, File, OpenOptions};
    use std::io::{ErrorKind, Read, Write};
    use std::path::Path;
    use tracing::trace;

    pub fn write<N, K, V>(db: &Config, namespace: N, key: K, value: V) -> SharedResult<()>
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
        f.write_all(data)?;
        Ok(fs::rename(path, key_path(db, &namespace, &key))?)
    }

    pub fn read<N, K, V>(db: &Config, namespace: N, key: K) -> SharedResult<Option<V>>
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

    pub fn delete<N, K>(db: &Config, namespace: N, key: K) -> SharedResult<()>
    where
        N: AsRef<[u8]>,
        K: AsRef<[u8]>,
    {
        let path_str = key_path(db, namespace, key);
        let path = Path::new(&path_str);
        trace!("delete\t{}", &path_str);
        if path.exists() {
            remove_file(path)?;
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
}
