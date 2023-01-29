use itertools::Itertools;
use lockbook_shared::document_repo::DocActivityMetrics;
use lockbook_shared::document_repo::StatisticValue;
use lockbook_shared::document_repo::Stats;

use crate::Requester;
use crate::{CoreResult, RequestContext};

use uuid::Uuid;
impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut scores: Vec<(Uuid, DocActivityMetrics)> = vec![];

        self.tx
            .docs_events
            .get_all()
            .iter()
            .for_each(|(key, doc_events)| {
                scores.push((*key, doc_events.iter().get_activity_metrics()));
            });

        //normalize
        Self::normalize(&mut scores);

        scores.sort_by(|a, b| {
            DocActivityMetrics::rank_doc_activity(&a.1)
                .cmp(&DocActivityMetrics::rank_doc_activity(&b.1))
        });

        Ok(scores.into_iter().map(|f| f.0).collect_vec())
    }

    fn normalize(scores: &mut [(Uuid, DocActivityMetrics)]) {
        let mut docs_avg_read_timestamps = scores
            .iter_mut()
            .map(|f| f.1.avg_read_timestamp)
            .collect_vec();
        docs_avg_read_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_avg_write_timestamps = scores
            .iter_mut()
            .map(|f| f.1.avg_write_timestamp)
            .collect_vec();
        docs_avg_write_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_read_count = scores.iter_mut().map(|f| f.1.read_count).collect_vec();
        docs_read_count.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_write_count = scores.iter_mut().map(|f| f.1.write_count).collect_vec();
        docs_write_count.sort_by(|a, b| a.raw.cmp(&b.raw));

        for (_, feat) in scores.iter_mut() {
            StatisticValue::normalize(
                &mut feat.avg_read_timestamp,
                *docs_avg_read_timestamps.last().unwrap(),
                *docs_avg_read_timestamps.first().unwrap(),
            );
            StatisticValue::normalize(
                &mut feat.avg_write_timestamp,
                *docs_avg_write_timestamps.last().unwrap(),
                *docs_avg_write_timestamps.first().unwrap(),
            );
            StatisticValue::normalize(
                &mut feat.read_count,
                *docs_read_count.last().unwrap(),
                *docs_read_count.first().unwrap(),
            );
            StatisticValue::normalize(
                &mut feat.write_count,
                *docs_write_count.last().unwrap(),
                *docs_write_count.first().unwrap(),
            );
        }
    }
}
