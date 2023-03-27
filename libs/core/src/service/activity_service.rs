use crate::CoreState;
use crate::LbResult;
use crate::Requester;
use db_rs::DbError;
use lockbook_shared::document_repo::DocActivityMetrics;
use lockbook_shared::document_repo::DocEvent;
use lockbook_shared::document_repo::StatisticValueRange;
use lockbook_shared::document_repo::Stats;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn suggested_docs(&mut self) -> LbResult<Vec<Uuid>> {
        let mut scores = self
            .db
            .doc_events
            .data()
            .iter()
            .get_activity_metrics()
            .iter_mut()
            .normalize();

        scores.sort_by(|a, b| DocActivityMetrics::rank(a).cmp(&DocActivityMetrics::rank(b)));

        Ok(scores.iter().map(|f| f.id).collect())
    }

    pub(crate) fn add_doc_event(&mut self, event: DocEvent) -> Result<(), DbError> {
        let max_stored_events = 1000;
        let events = &self.db.doc_events;

        if events.data().len() > max_stored_events {
            self.db.doc_events.pop()?;
        }
        self.db.doc_events.push(event)
    }
}

trait Normalizer {
    fn normalize(&mut self) -> Vec<DocActivityMetrics>;
}
impl<'a, T> Normalizer for T
where
    T: Iterator<Item = &'a mut DocActivityMetrics>,
{
    fn normalize(&mut self) -> Vec<DocActivityMetrics> {
        let read_count_range = StatisticValueRange {
            max: self.map(|f| f.read_count).max().unwrap(),
            min: self.map(|f| f.read_count).min().unwrap(),
        };
        let write_count_range = StatisticValueRange {
            max: self.map(|f| f.write_count).max().unwrap(),
            min: self.map(|f| f.write_count).min().unwrap(),
        };

        let latest_read_range = StatisticValueRange {
            max: self.map(|f| f.latest_read_timestamp).max().unwrap(),
            min: self.map(|f| f.latest_read_timestamp).min().unwrap(),
        };
        let latest_write_range = StatisticValueRange {
            max: self.map(|f| f.latest_write_timestamp).max().unwrap(),
            min: self.map(|f| f.latest_write_timestamp).min().unwrap(),
        };
        self.for_each(|f| {
            f.read_count.normalize(read_count_range);
            f.write_count.normalize(write_count_range);
            f.latest_read_timestamp.normalize(latest_read_range);
            f.latest_write_timestamp.normalize(latest_write_range);
        });
        vec![]
    }
}
