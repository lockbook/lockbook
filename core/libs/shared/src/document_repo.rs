use crate::core_config::Config;
use crate::crypto::*;
use crate::{SharedError, SharedResult};
use std::fs::{self, create_dir_all, remove_file, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RepoSource {
    Local, // files with local edits applied
    Base,  // files at latest known state when client and server matched
}

impl RepoSource {
    fn disk_name(&self) -> &'static str {
        match &self {
            RepoSource::Local => "changed_local_documents",
            RepoSource::Base => "all_base_documents",
        }
    }
}

pub fn namespace_path(db: &Config, namespace: &RepoSource) -> String {
    format!("{}/{}", db.writeable_path, namespace.disk_name())
}

fn key_path(db: &Config, namespace: &RepoSource, key: &Uuid) -> String {
    format!("{}/{}", namespace_path(db, namespace), key)
}

#[instrument(level = "debug", skip(config, document), err(Debug))]
pub fn insert(
    config: &Config, source: RepoSource, id: &Uuid, document: &EncryptedDocument,
) -> SharedResult<()> {
    let value = &bincode::serialize(document)?;
    let path_str = key_path(config, &source, id) + ".pending";
    let path = Path::new(&path_str);
    trace!("write\t{} {:?} bytes", &path_str, value.len());
    create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    f.write_all(value)?;
    Ok(fs::rename(path, key_path(config, &source, id))?)
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn get(config: &Config, source: RepoSource, id: &Uuid) -> SharedResult<EncryptedDocument> {
    maybe_get(config, source, id)?.ok_or(SharedError::FileNonexistent)
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(
    config: &Config, source: RepoSource, id: &Uuid,
) -> SharedResult<Option<EncryptedDocument>> {
    let path_str = key_path(config, &source, id);
    let path = Path::new(&path_str);
    trace!("read\t{}", &path_str);
    let maybe_data: Option<Vec<u8>> = match File::open(path) {
        Ok(mut f) => {
            let mut buffer: Vec<u8> = Vec::new();
            f.read_to_end(&mut buffer)?;
            Some(buffer)
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => None,
            _ => return Err(err.into()),
        },
    };

    Ok(match maybe_data {
        Some(data) => bincode::deserialize(&data).map(Some)?,
        None => None,
    })
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn delete(config: &Config, source: RepoSource, id: &Uuid) -> SharedResult<()> {
    let path_str = key_path(config, &source, id);
    let path = Path::new(&path_str);
    trace!("delete\t{}", &path_str);
    if path.exists() {
        remove_file(path)?;
    }

    Ok(())
}
