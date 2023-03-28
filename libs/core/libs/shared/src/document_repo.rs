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
    pub normalized: Option<f64>,
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
        let normalized = (self.raw - range.min.raw)
            .checked_div(range.max.raw - range.min.raw)
            .unwrap_or(1);
        self.normalized = Some(normalized as f64);
    }
}
/// DocActivityMetrics stores key document activity features, which are used to recommend relevant documents to the user.
/// Here's a walkthrough of the recommendation procedure: collect 1k most recent document events (write/read), use that activity to construct a DocActivtyMetrics struct for each document. Min-max normalizes the activity features, then rank the documents.
#[derive(Default, Copy, Clone, PartialEq)]
pub struct DocActivityMetrics {
    pub id: Uuid,
    /// the latest epoch timestamp that the user read a document
    pub last_read_timestamp: StatisticValue,
    /// the latest epoch timestamp that the user wrote a document
    pub last_write_timestamp: StatisticValue,
    /// the total number of times that a user reads a document
    pub read_count: StatisticValue,
    /// the total number of times that a user wrote a document
    pub write_count: StatisticValue,
}

impl Eq for DocActivityMetrics {}
impl Ord for DocActivityMetrics {
    fn cmp(&self, other: &Self) -> Ordering {
        other.score().cmp(&self.score())
    }
}

impl PartialOrd for DocActivityMetrics {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(other.score().cmp(&self.score()))
    }
}

impl DocActivityMetrics {
    pub fn score(&self) -> i64 {
        let timestamp_weight = 70;
        let io_count_weight = 30;

        (self.last_read_timestamp.normalized.unwrap_or_default()
            + self.last_write_timestamp.normalized.unwrap_or_default()) as i64
            * timestamp_weight
            + (self.read_count.normalized.unwrap_or_default()
                + self.write_count.normalized.unwrap_or_default()) as i64
                * io_count_weight
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

        for (id, events) in set {
            let read_events = events.iter().filter(|e| matches!(e, DocEvent::Read(_, _)));

            let last_read = read_events
                .clone()
                .max_by(|x, y| x.timestamp().cmp(&y.timestamp()));

            let last_read = match last_read {
                None => 0,
                Some(x) => x.timestamp(),
            };

            let write_events = events.iter().filter(|e| matches!(e, DocEvent::Write(_, _)));

            let last_write = write_events
                .clone()
                .max_by(|x, y| x.timestamp().cmp(&y.timestamp()));
            let last_write = match last_write {
                None => 0,
                Some(x) => x.timestamp(),
            };

            let metrics = DocActivityMetrics {
                id,
                last_read_timestamp: StatisticValue { raw: last_read, normalized: None },
                last_write_timestamp: StatisticValue { raw: last_write, normalized: None },
                read_count: StatisticValue { raw: read_events.count() as i64, normalized: None },
                write_count: StatisticValue { raw: write_events.count() as i64, normalized: None },
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
