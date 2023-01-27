use crate::core_config::Config;
use crate::crypto::*;
use crate::file_metadata::DocumentHmac;
use crate::{SharedError, SharedResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocEvents {
    Read(i64),
    Write(i64),
}
#[derive(Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct StatisticValue {
    pub raw: i64,
    pub normalized: Option<f64>,
}

impl StatisticValue {
    pub fn normalize(&mut self, max: StatisticValue, min: StatisticValue) {
        self.normalized = Some((self.raw - min.raw) as f64 / (max.raw - min.raw) as f64)
    }
}

#[derive(Default)]
pub struct DocActivityScore {
    pub avg_read_timestamp: StatisticValue,
    pub avg_write_timestamp: StatisticValue,
    pub read_count: StatisticValue,
    pub write_count: StatisticValue,
}

pub trait Stats {
    fn score(self) -> DocActivityScore;
}
impl<'a, T> Stats for T
where
    T: Iterator<Item = &'a DocEvents>,
{
    fn score(self) -> DocActivityScore {
        let mut read_activity: Vec<i64> = vec![];
        let mut write_activity: Vec<i64> = vec![];
        let mut write_sum = 0;
        let mut read_sum = 0;

        self.for_each(|event| match event {
            DocEvents::Read(timestamp) => {
                read_activity.push(*timestamp);
                read_sum += timestamp;
            }
            DocEvents::Write(timestamp) => {
                write_activity.push(*timestamp);
                write_sum += timestamp;
            }
        });

        let avg_read_timestamp = read_sum / read_activity.len() as i64;
        let avg_write_timestamp = write_sum / write_activity.len() as i64;

        DocActivityScore {
            avg_read_timestamp: StatisticValue { raw: avg_read_timestamp, normalized: None },
            avg_write_timestamp: StatisticValue { raw: avg_write_timestamp, normalized: None },
            read_count: StatisticValue { raw: read_activity.len() as i64, normalized: None },
            write_count: StatisticValue { raw: write_activity.len() as i64, normalized: None },
        }
    }
}

pub fn namespace_path(writeable_path: &str) -> String {
    format!("{}/documents", writeable_path)
}

pub fn key_path(writeable_path: &str, key: &Uuid, hmac: &DocumentHmac) -> String {
    let hmac = base64::encode_config(hmac, base64::URL_SAFE);
    format!("{}/{}-{}", namespace_path(writeable_path), key, hmac)
}

#[instrument(level = "debug", skip(config, document), err(Debug))]
pub fn insert(
    config: &Config, id: &Uuid, hmac: Option<&DocumentHmac>, document: &EncryptedDocument,
) -> SharedResult<()> {
    if let Some(hmac) = hmac {
        let value = &bincode::serialize(document)?;
        let path_str = key_path(&config.writeable_path, id, hmac) + ".pending";
        let path = Path::new(&path_str);
        trace!("write\t{} {:?} bytes", &path_str, value.len());
        fs::create_dir_all(path.parent().unwrap())?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        f.write_all(value)?;
        Ok(fs::rename(path, key_path(&config.writeable_path, id, hmac))?)
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
        let path_str = key_path(&config.writeable_path, id, hmac);
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
        let path_str = key_path(&config.writeable_path, id, hmac);
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
    let dir_path = namespace_path(&config.writeable_path);
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
