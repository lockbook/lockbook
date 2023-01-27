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
        let mut doc_scores: HashMap<Uuid, DocActivityScore> = HashMap::new();

        self.tx
            .docs_events
            .get_all()
            .iter()
            .for_each(|(key, doc_events)| {
                doc_scores.insert(*key, doc_events.iter().score());
            });

        //normalize
        let mut docs_avg_read_timestamps = doc_scores
            .values()
            .map(|f| f.avg_read_timestamp)
            .collect_vec();
        docs_avg_read_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_avg_write_timestamps = doc_scores
            .values()
            .map(|f| f.avg_write_timestamp)
            .collect_vec();
        docs_avg_write_timestamps.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_read_count = doc_scores.values().map(|f| f.read_count).collect_vec();
        docs_read_count.sort_by(|a, b| a.raw.cmp(&b.raw));

        let mut docs_write_count = doc_scores.values().map(|f| f.write_count).collect_vec();
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

        Ok(vec![])
    }
}

// pub trait Range<'a>: Iterator {
//     fn range(self) -> Self::Item;
// }

// impl<'a, F, T> Range<'_> for T
// where
//     T: Iterator<Item = &'a F>,
//     F: std::borrow::Borrow<f64> + Ord + 'a,
//     &'a F: std::ops::Sub<Output = &'a F>,
// {
//     fn range(self) -> &'a F {
//         let mut vec: Vec<&'a F> = self.collect_vec();
//         vec.sort();
//         *vec.last().unwrap() - *vec.first().unwrap()
//     }
// }

// pub trait Range: Iterator {
//     fn range(self) -> i64;
// }

// impl<'a, T> Range for T
// where
//     T: Iterator<Item = &'a i64>,
// {
//     fn range(self) -> i64 {
//         let mut vec: Vec<&'a i64> = self.collect_vec();
//         vec.sort();
//         *vec.last().unwrap() - *vec.first().unwrap()
//     }
// }
