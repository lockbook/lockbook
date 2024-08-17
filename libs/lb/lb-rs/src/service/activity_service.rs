use std::cmp;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::CoreState;
use crate::LbResult;
use crate::Requester;
use crate::shared::document_repo::DocumentService;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn suggested_docs(&mut self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        let mut scores = self.db.doc_events.get().iter().get_activity_metrics();
        self.normalize(&mut scores);

        scores.sort_unstable_by_key(|b| cmp::Reverse(b.score(settings)));

        Ok(scores.iter().map(|f| f.id).collect())
    }

    pub(crate) fn add_doc_event(&mut self, event: DocEvent) -> LbResult<()> {
        let max_stored_events = 1000;
        let events = &self.db.doc_events;

        if events.get().len() > max_stored_events {
            self.db.doc_events.remove(0)?;
        }
        self.db.doc_events.push(event)?;
        Ok(())
    }

    pub(crate) fn normalize(&mut self, docs: &mut [DocActivityMetrics]) {
        let read_count_range = StatisticValueRange {
            max: docs.iter().map(|f| f.read_count).max().unwrap_or_default(),
            min: docs.iter().map(|f| f.read_count).min().unwrap_or_default(),
        };

        let write_count_range = StatisticValueRange {
            max: docs.iter().map(|f| f.write_count).max().unwrap_or_default(),
            min: docs.iter().map(|f| f.write_count).min().unwrap_or_default(),
        };

        let last_read_range = StatisticValueRange {
            max: docs
                .iter()
                .map(|f| f.last_read_timestamp)
                .max()
                .unwrap_or_default(),
            min: docs
                .iter()
                .map(|f| f.last_read_timestamp)
                .min()
                .unwrap_or_default(),
        };
        let last_write_range = StatisticValueRange {
            max: docs
                .iter()
                .map(|f| f.last_write_timestamp)
                .max()
                .unwrap_or_default(),
            min: docs
                .iter()
                .map(|f| f.last_write_timestamp)
                .min()
                .unwrap_or_default(),
        };

        docs.iter_mut().for_each(|f| {
            f.read_count.normalize(read_count_range);
            f.write_count.normalize(write_count_range);
            f.last_read_timestamp.normalize(last_read_range);
            f.last_write_timestamp.normalize(last_write_range);
        });
    }
}

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

#[derive(Debug, Copy, Clone)]
pub struct RankingWeights {
    /// the freshness of a doc as determined by the last activity
    pub temporality: i64,
    /// the amount of write and read on a doc
    pub io: i64,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self { temporality: 60, io: 40 }
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
        let mut range_distance = range.max.raw - range.min.raw;
        if range_distance == 0 {
            range_distance = 1
        };
        let normalized = (self.raw - range.min.raw) as f64 / range_distance as f64;
        self.normalized = Some(normalized);
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

impl DocActivityMetrics {
    pub fn score(&self, weights: RankingWeights) -> i64 {
        let timestamp_weight = weights.temporality;
        let io_count_weight = weights.io;

        let temporality_score = (self.last_read_timestamp.normalized.unwrap_or_default()
            + self.last_write_timestamp.normalized.unwrap_or_default())
            * timestamp_weight as f64;

        let io_score = (self.read_count.normalized.unwrap_or_default()
            + self.write_count.normalized.unwrap_or_default())
            * io_count_weight as f64;

        (io_score + temporality_score).ceil() as i64
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
