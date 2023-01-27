use lockbook_shared::clock::get_time;
use lockbook_shared::document_repo::DocActivityScore;
use lockbook_shared::document_repo::DocEvents;
use lockbook_shared::document_repo::Stats;
use std::collections::HashMap;
use std::iter::Sum;

use crate::Requester;
use crate::{CoreResult, RequestContext};

use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut score_table: HashMap<Uuid, DocActivityScore> = HashMap::new();
        let now = get_time().0;

        // populate from read
        for (key, events) in self.tx.doc_events.get_all() {
            events.iter().score();
            // write_events.map(|f| {
            //     if let DocEvents::Write(timestamp) = f {
            //         timestamp - now
            //     }
            // });
        }

        // // populate from write
        // for (key, value) in self.tx.write_activity.get_all() {
        //     let mut table_entry = score_table.remove(key).unwrap_or_default();
        //     table_entry.write_count = value.len() as i64;
        //     table_entry.avg_write_timestamp = value.iter().sum::<i64>() / value.len() as i64;
        //     score_table.insert(*key, table_entry);
        // }

        // // normalize
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
