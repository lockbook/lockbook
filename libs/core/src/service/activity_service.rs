use lockbook_shared::clock::get_time;
use lockbook_shared::document_repo::DocActivityScore;
use lockbook_shared::document_repo::Stats;
use std::collections::HashMap;
use std::ops::Sub;

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
                doc_scores.insert(*key, doc_events.into_iter().score());
            });

        //normalize
        let millis_read_range = doc_scores.values();

        // let millis_write_range = doc_scores.values().map(|f| f.avg_write_timestamp).range();

        // let millis_write_range = &doc_scores.values().map(|f| f.avg_write_timestamp).range();
        // let read_range = doc_scores.values().map(|f| f.read_count).range();
        // let write_range = &doc_scores.values().map(|f| f.write_count).range();

        // for (_, feat) in doc_scores.iter_mut() {
        //     feat.avg_read_timestamp /= millis_read_range;
        //     feat.avg_write_timestamp /= millis_write_range;
        //     feat.read_count /= read_range;
        //     feat.write_count /= write_range;
        // }

        Ok(vec![])
    }
}

trait Range: Iterator {
    fn range(self) -> Self::Item;
}
impl<'a,T, F> Range for T
where
    T: Iterator<Item = F>,
    F: Ord + Sub<Output = F>,
{
    fn range(self) -> Self::Item {
        self.max().unwrap() - self.min().unwrap()
    }
}
