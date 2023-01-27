use lockbook_shared::clock::get_time;
use lockbook_shared::document_repo::DocActivityScore;
use lockbook_shared::document_repo::Stats;
use std::collections::HashMap;

use crate::Requester;
use crate::{CoreResult, RequestContext};

use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut score_table: HashMap<Uuid, DocActivityScore> = HashMap::new();
        let now = get_time().0;

        for (key, doc_events) in self.tx.docs_events.get_all() {
            let doc_score = (*doc_events).into_iter().score();
        }

        //normalize
        // let millis_read_range = score_table.values().map(|f| f.avg_read_timestamp).range();

        // let millis_write_range = score_table.values().map(|f| f.avg_write_timestamp).range();

        // let millis_write_range = &score_table.values().map(|f| f.avg_write_timestamp).range();
        // let read_range = score_table.values().map(|f| f.read_count).range();
        // let write_range = &score_table.values().map(|f| f.write_count).range();

        // for (_, feat) in score_table.iter_mut() {
        //     feat.avg_read_timestamp /= millis_read_range;
        //     feat.avg_write_timestamp /= millis_write_range;
        //     feat.read_count /= read_range;
        //     feat.write_count /= write_range;
        // }

        Ok(vec![])
    }

    fn mean(numbers: &[i64]) -> i64 {
        if numbers.is_empty() {
            return 0;
        }

        numbers.iter().max().unwrap() - numbers.iter().min().unwrap()
    }
}

// trait Stats: Iterator {
//     fn mean(self) -> i64;
// }
// impl<T, F> Stats for T
// where
//     T: Iterator<Item = DocEvents>,
// {
//     fn mean(self) -> i64 {
//         1
//     }
// }
