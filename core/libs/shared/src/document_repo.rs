use crate::core_config::Config;
use crate::crypto::*;
use crate::file_metadata::DocumentHmac;
use crate::{SharedError, SharedResult};
use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

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

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn retain(config: &Config, file_hmacs: HashSet<(&Uuid, &DocumentHmac)>) -> SharedResult<()> {
    let dir_path = namespace_path(config);
    fs::create_dir_all(&dir_path)?;
    let entries = fs::read_dir(&dir_path)?;
    for entry in entries {
        let path = entry?.path();
        let (id_str, hmac_str) = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(SharedError::Unexpected("document disk file name malformed"))?
            .split_at(36); // Uuid's are 36 characters long in string form
        let id = Uuid::parse_str(id_str)
            .map_err(|_| SharedError::Unexpected("document disk file name malformed"))?;
        let hmac: DocumentHmac = base64::decode_config(
            hmac_str
                .strip_prefix('-')
                .ok_or(SharedError::Unexpected("document disk file name malformed"))?,
            base64::URL_SAFE,
        )
        .map_err(|_| SharedError::Unexpected("document disk file name malformed"))?
        .try_into()
        .map_err(|_| SharedError::Unexpected("document disk file name malformed"))?;
        if !file_hmacs.contains(&(&id, &hmac)) {
            delete(config, &id, Some(&hmac))?;
        }
    }

    Ok(())
}
