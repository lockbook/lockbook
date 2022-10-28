use crate::core_config::Config;
use crate::crypto::*;
use crate::file_metadata::DocumentHmac;
use crate::{SharedError, SharedResult};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepoSource {
    Local, // files with local edits applied
    Base,  // files at latest known state when client and server matched
}

pub fn namespace_path(db: &Config) -> String {
    format!("{}/documents", db.writeable_path)
}

fn key_path(db: &Config, key: &Uuid, hmac: &DocumentHmac) -> String {
    let hmac = base64::encode_config(hmac, base64::URL_SAFE);
    format!("{}/{}-{}", namespace_path(db), key, hmac)
}

#[instrument(level = "debug", skip(config, document), err(Debug))]
pub fn insert(
    config: &Config, id: &Uuid, hmac: Option<&DocumentHmac>, document: &EncryptedDocument,
) -> SharedResult<()> {
    if let Some(hmac) = hmac {
        let value = &bincode::serialize(document)?;
        let path_str = key_path(config, id, hmac) + ".pending";
        let path = Path::new(&path_str);
        trace!("write\t{} {:?} bytes", &path_str, value.len());
        fs::create_dir_all(path.parent().unwrap())?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        f.write_all(value)?;
        Ok(fs::rename(path, key_path(config, id, hmac))?)
    } else {
        Ok(())
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn get(
    config: &Config, id: &Uuid, hmac: Option<&DocumentHmac>,
) -> SharedResult<EncryptedDocument> {
    maybe_get(config, id, hmac)?.ok_or(SharedError::FileNonexistent)
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(
    config: &Config, id: &Uuid, hmac: Option<&DocumentHmac>,
) -> SharedResult<Option<EncryptedDocument>> {
    if let Some(hmac) = hmac {
        let path_str = key_path(config, id, hmac);
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
    } else {
        Ok(None)
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn delete(config: &Config, id: &Uuid, hmac: Option<&DocumentHmac>) -> SharedResult<()> {
    if let Some(hmac) = hmac {
        let path_str = key_path(config, id, hmac);
        let path = Path::new(&path_str);
        trace!("delete\t{}", &path_str);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}
