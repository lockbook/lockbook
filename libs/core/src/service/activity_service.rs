use crate::CoreResult;
use crate::CoreState;
use crate::Requester;
use itertools::Itertools;
use lockbook_shared::document_repo::DocActivityMetrics;
use lockbook_shared::document_repo::StatisticValueRange;
use lockbook_shared::document_repo::Stats;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut scores: Vec<(Uuid, DocActivityMetrics)> = vec![];

        self.db
            .doc_events
            .data()
            .iter()
            .for_each(|(key, doc_events)| {
                scores.push((*key, doc_events.iter().get_activity_metrics()));
            });

        //normalize
        Self::normalize(&mut scores);

        scores.sort_by(|a, b| DocActivityMetrics::rank(&a.1).cmp(&DocActivityMetrics::rank(&b.1)));

        Ok(scores.into_iter().map(|f| f.0).collect_vec())
    }

    fn normalize(scores: &mut [(Uuid, DocActivityMetrics)]) {
        let docs_avg_read_timestamps = StatisticValueRange {
            max: scores
                .iter_mut()
                .map(|f| f.1.avg_read_timestamp)
                .max()
                .unwrap(),
            min: scores
                .iter_mut()
                .map(|f| f.1.avg_read_timestamp)
                .min()
                .unwrap(),
        };

        let docs_avg_write_timestamps = StatisticValueRange {
            max: scores
                .iter_mut()
                .map(|f| f.1.avg_write_timestamp)
                .max()
                .unwrap(),
            min: scores
                .iter_mut()
                .map(|f| f.1.avg_write_timestamp)
                .min()
                .unwrap(),
        };

        let docs_read_count = StatisticValueRange {
            max: scores.iter_mut().map(|f| f.1.read_count).max().unwrap(),
            min: scores.iter_mut().map(|f| f.1.read_count).min().unwrap(),
        };

        let docs_write_count = StatisticValueRange {
            max: scores.iter_mut().map(|f| f.1.write_count).max().unwrap(),
            min: scores.iter_mut().map(|f| f.1.write_count).min().unwrap(),
        };

        for (_, feat) in scores.iter_mut() {
            feat.avg_read_timestamp.normalize(docs_avg_read_timestamps);
            feat.avg_write_timestamp
                .normalize(docs_avg_write_timestamps);
            feat.read_count.normalize(docs_read_count);
            feat.write_count.normalize(docs_write_count);
        }
    }
}
