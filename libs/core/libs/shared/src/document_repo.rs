use crate::core_config::Config;
use crate::file_metadata::DocumentHmac;
use crate::SharedResult;
use crate::{crypto::*, SharedErrorKind};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fs::{self, File, OpenOptions};
use std::hash::Hash;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use tracing::*;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialEq, PartialOrd, Eq, Hash)]
pub enum DocEvent {
    Read(Uuid, i64),
    Write(Uuid, i64),
}
impl DocEvent {
    pub fn timestamp(&self) -> i64 {
        match *self {
            DocEvent::Read(_, timestamp) => timestamp,
            DocEvent::Write(_, timestamp) => timestamp,
        }
    }
    pub fn id(&self) -> Uuid {
        match *self {
            DocEvent::Read(id, _) => id,
            DocEvent::Write(id, _) => id,
        }
    }
}

#[derive(Default, Copy, Clone, PartialEq)]
pub struct StatisticValue {
    pub raw: i64,
    pub normalized: f64,
}
impl StatisticValue {
    fn from_raw(raw: i64) -> StatisticValue {
        StatisticValue { raw, normalized: f64::default() }
    }
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
/// DocActivityMetrics stores key document activity features, which are used to recommend relevant documents to the user.
/// Here's a walkthrough of the recommendation procedure: collect 1k most recent document events (write/read), use that activity to construct a DocActivtyMetrics struct for each document. Min-max normalizes the activity features, then rank the documents.
/// latest_read_timestamp: the latest epoch timestamp that the user read a document
/// latest_write_timestamp: the latest epoch timestamp that the user wrote a document
/// the total number of times that a user reads a document
/// the total number of times that a user wrote a document
#[derive(Default, Copy, Clone)]
pub struct DocActivityMetrics {
    pub id: Uuid,
    pub latest_read_timestamp: StatisticValue,
    pub latest_write_timestamp: StatisticValue,
    pub read_count: StatisticValue,
    pub write_count: StatisticValue,
}
impl DocActivityMetrics {
    pub fn rank(&self) -> i64 {
        (self.latest_read_timestamp.normalized + self.latest_write_timestamp.normalized) as i64 * 70
            + (self.read_count.normalized + self.write_count.normalized) as i64 * 30
    }
}
pub trait Stats {
    fn get_activity_metrics(self) -> Vec<DocActivityMetrics>;
}
impl<'a, T> Stats for T
where
    T: Iterator<Item = &'a DocEvent>,
{
    fn get_activity_metrics(self) -> Vec<DocActivityMetrics> {
        let mut result = Vec::new();

        let mut set = HashMap::new();
        for event in self {
            match set.get_mut(&event.id()) {
                None => {
                    set.insert(event.id(), vec![event]);
                }
                Some(events) => {
                    events.push(event);
                }
            }
        }

        for pair in set {
            let read_events: Vec<&&DocEvent> = pair
                .1
                .iter()
                .filter(|e| matches!(e, DocEvent::Read(_, _)))
                .collect();
            let latest_read = read_events
                .iter()
                .max_by(|x, y| x.timestamp().cmp(&y.timestamp()));
            let latest_read = match latest_read {
                None => 0,
                Some(x) => x.timestamp(),
            };

            let write_events: Vec<&&DocEvent> = pair
                .1
                .iter()
                .filter(|e| matches!(e, DocEvent::Write(_, _)))
                .collect();

            let latest_write = write_events
                .iter()
                .max_by(|x, y| x.timestamp().cmp(&y.timestamp()));
            let latest_write = match latest_write {
                None => 0,
                Some(x) => x.timestamp(),
            };

            let metrics = DocActivityMetrics {
                id: pair.0,
                latest_read_timestamp: StatisticValue::from_raw(latest_read),
                latest_write_timestamp: StatisticValue::from_raw(latest_write),
                read_count: StatisticValue::from_raw(read_events.len() as i64),
                write_count: StatisticValue::from_raw(write_events.len() as i64),
            };
            result.push(metrics);
        }

        result
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
