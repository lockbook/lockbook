use itertools::Itertools;
use lockbook_shared::document_repo::DocActivityScore;
use lockbook_shared::document_repo::StatisticValue;
use lockbook_shared::document_repo::Stats;
use std::collections::HashMap;

use crate::Requester;
use crate::{CoreResult, RequestContext};

use uuid::Uuid;
impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut doc_scores: Vec<(Uuid, DocActivityScore)> = vec![];

        self.tx
            .docs_events
            .get_all()
            .iter()
            .for_each(|(key, doc_events)| {
                doc_scores.push((*key, doc_events.iter().score()));
            });

        //normalize
        let mut docs_avg_read_timestamps = doc_scores
            .iter_mut()
            .map(|f| f.1.avg_read_timestamp)
            .collect_vec();
        docs_avg_read_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_avg_write_timestamps = doc_scores
            .iter_mut()
            .map(|f| f.1.avg_write_timestamp)
            .collect_vec();
        docs_avg_write_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_read_count = doc_scores.iter_mut().map(|f| f.1.read_count).collect_vec();
        docs_read_count.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_write_count = doc_scores.iter_mut().map(|f| f.1.write_count).collect_vec();
        docs_write_count.sort_by(|a, b| a.raw.cmp(&b.raw));

        for (_, feat) in doc_scores.iter_mut() {
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

        doc_scores
            .sort_by(|a, b| DocActivityScore::score(&a.1).cmp(&DocActivityScore::score(&b.1)));

        Ok(doc_scores.into_iter().map(|f| f.0).collect_vec())
    }
}
