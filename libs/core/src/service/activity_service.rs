use crate::CoreState;
use crate::LbResult;
use crate::Requester;
use lockbook_shared::document_repo::DocActivityMetrics;
use lockbook_shared::document_repo::DocEvent;
use lockbook_shared::document_repo::StatisticValueRange;
use lockbook_shared::document_repo::Stats;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn suggested_docs(&mut self) -> LbResult<Vec<Uuid>> {
        let mut scores = self.db.doc_events.data().iter().get_activity_metrics();
        self.normalize(&mut scores);
        scores.sort();

        Ok(scores.iter().map(|f| f.id).collect())
    }

    pub(crate) fn add_doc_event(&mut self, event: DocEvent) -> LbResult<()> {
        let max_stored_events = 1000;
        let events = &self.db.doc_events;

        if events.data().len() > max_stored_events {
            self.db.doc_events.pop()?;
        }
        self.db.doc_events.push(event)?;
        Ok(())
    }

    pub(crate) fn normalize(
        &mut self, docs: &mut Vec<DocActivityMetrics>,
    ) -> Vec<DocActivityMetrics> {
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

        docs.iter().map(|f| *f).collect()
    }
}
