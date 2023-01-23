use itertools::Itertools;
use std::collections::HashMap;

use crate::Requester;
use crate::{CoreResult, RequestContext};
use lockbook_shared::clock::get_time;
use uuid::Uuid;

#[derive(Default)]
struct Features {
    millis_since_read: i64,
    millis_since_written: i64,
    read_count: i64,
    write_count: i64,
}

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn suggested_docs(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut activity_table: HashMap<Uuid, Features> = HashMap::new();

        let now = get_time().0;

        // populate from read
        for (key, value) in self.tx.read_activity.get_all() {
            let mut table_entry = activity_table.remove(key).unwrap_or_default();
            table_entry.read_count = value.len() as i64;
            table_entry.millis_since_read = now - value.iter().max().copied().unwrap_or(i64::MAX);
            activity_table.insert(*key, table_entry);
        }

        // populate from write
        for (key, value) in self.tx.write_activity.get_all() {
            let mut table_entry = activity_table.remove(key).unwrap_or_default();
            table_entry.write_count = value.len() as i64;
            table_entry.millis_since_written =
                now - value.iter().max().copied().unwrap_or(i64::MAX);
            activity_table.insert(*key, table_entry);
        }

        // normalize
        let millis_read_range = Self::range(
            &activity_table
                .values()
                .map(|f| f.millis_since_read)
                .collect_vec(),
        );
        let millis_write_range = Self::range(
            &activity_table
                .values()
                .map(|f| f.millis_since_written)
                .collect_vec(),
        );
        let read_range = Self::range(&activity_table.values().map(|f| f.read_count).collect_vec());
        let write_range =
            Self::range(&activity_table.values().map(|f| f.write_count).collect_vec());

        for (_, feat) in activity_table.iter_mut() {
            feat.millis_since_read /= millis_read_range;
            feat.millis_since_written /= millis_write_range;
            feat.read_count /= read_range;
            feat.write_count /= write_range;
        }

        Ok(vec![])
    }

    fn range(numbers: &[i64]) -> i64 {
        if numbers.is_empty() {
            return 0;
        }

        numbers.iter().max().unwrap() - numbers.iter().min().unwrap()
    }
}
