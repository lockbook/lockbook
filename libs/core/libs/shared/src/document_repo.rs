use crate::core_config::Config;
use crate::file_metadata::DocumentHmac;
use crate::SharedResult;
use crate::{crypto::*, SharedErrorKind};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialEq, PartialOrd, Eq, Hash)]
pub enum DocEvents {
    Read(i64),
    Write(i64),
}
impl DocEvents {
    pub fn timestamp(&self) -> i64 {
        match *self {
            DocEvents::Read(x) => x,
            DocEvents::Write(x) => x,
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct StatisticValue {
    pub raw: i64,
    pub normalized: f64,
}
impl Ord for StatisticValue {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.raw).cmp(&other.raw)
    }
}

impl PartialOrd for StatisticValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for StatisticValue {
    fn eq(&self, other: &Self) -> bool {
        (self.raw, &self.normalized) == (other.raw, &other.normalized)
    }
}

impl Eq for StatisticValue {}

#[derive(Clone, Copy)]
pub struct StatisticValueRange {
    pub max: StatisticValue,
    pub min: StatisticValue,
}
impl StatisticValue {
    pub fn normalize(&mut self, range: StatisticValueRange) {
        self.normalized = (self.raw - range.min.raw) as f64 / (range.max.raw - range.min.raw) as f64
    }
}

#[derive(Default, Copy, Clone)]
pub struct DocActivityMetrics {
    pub avg_read_timestamp: StatisticValue,
    pub avg_write_timestamp: StatisticValue,
    pub read_count: StatisticValue,
    pub write_count: StatisticValue,
}
impl DocActivityMetrics {
    pub fn rank(&self) -> i64 {
        (self.avg_read_timestamp.normalized + self.avg_write_timestamp.normalized) as i64 * 70
            + (self.read_count.normalized + self.write_count.normalized) as i64 * 30
    }
}
pub trait Stats {
    fn get_activity_metrics(self) -> DocActivityMetrics;
}
impl<'a, T> Stats for T
where
    T: Iterator<Item = &'a DocEvents>,
{
    fn get_activity_metrics(self) -> DocActivityMetrics {
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

        DocActivityMetrics {
            avg_read_timestamp: StatisticValue {
                raw: avg_read_timestamp,
                normalized: f64::default(),
            },
            avg_write_timestamp: StatisticValue {
                raw: avg_write_timestamp,
                normalized: f64::default(),
            },
            read_count: StatisticValue {
                raw: read_activity.len() as i64,
                normalized: f64::default(),
            },
            write_count: StatisticValue {
                raw: write_activity.len() as i64,
                normalized: f64::default(),
            },
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
    maybe_get(config, id, hmac)?.ok_or_else(|| SharedErrorKind::FileNonexistent.into())
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
            .ok_or(SharedErrorKind::Unexpected("document disk file name malformed"))?
            .split_at(36); // Uuid's are 36 characters long in string form
        let id = Uuid::parse_str(id_str)
            .map_err(|_| SharedErrorKind::Unexpected("document disk file name malformed"))?;
        let hmac: DocumentHmac = base64::decode_config(
            hmac_str
                .strip_prefix('-')
                .ok_or(SharedErrorKind::Unexpected("document disk file name malformed"))?,
            base64::URL_SAFE,
        )
        .map_err(|_| SharedErrorKind::Unexpected("document disk file name malformed"))?
        .try_into()
        .map_err(|_| SharedErrorKind::Unexpected("document disk file name malformed"))?;
        if !file_hmacs.contains(&(&id, &hmac)) {
            delete(config, &id, Some(&hmac))?;
        }
    }

    Ok(())
}
