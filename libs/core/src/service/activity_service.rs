use std::collections::HashMap;

use crate::Requester;
use crate::{CoreResult, RequestContext};
use uuid::Uuid;

struct Features {
    timestamp: i64,
    read_count: usize,
    write_count: usize,
}

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn most_active_documents(&mut self) -> CoreResult<Vec<Uuid>> {
        let mut activity_table: HashMap<Uuid, Features> = HashMap::new();

        let active_files = self
            .tx
            .read_activity
            .get_all()
            .into_iter()
            .chain(self.tx.write_activity.get_all());

        active_files.for_each(|(k, _)| {
            let read_activity = *self.tx.read_activity.get(k).unwrap();
            let write_activity = *self.tx.write_activity.get(k).unwrap();

            let activity = [read_activity, write_activity].concat();
            let sum: i64 = activity.iter().sum();
            let avg = sum / activity.len() as i64;

            activity_table.insert(
                *k,
                Features {
                    timestamp: avg,
                    read_count: read_activity.len(),
                    write_count: write_activity.len(),
                },
            );
        });

        Self::normalize(&mut activity_table);
        Ok(vec![])
        //get max in all
    }

    fn normalize(target: &mut HashMap<Uuid, Features>) {
        let min_timestamp = i64::MAX;
        let max_timestamp = 0 as i64;

        let min_read_count = usize::MAX;
        let max_read_count = 0 as usize;

        let min_write_count = usize::MAX;
        let max_write_count = 0 as usize;

        for (k, v) in target {
            if v.timestamp > max_timestamp {
                max_timestamp = v.timestamp
            }
            if v.timestamp < min_timestamp {
                min_timestamp = v.timestamp
            }

            if v.read_count > max_read_count {
                max_read_count = v.read_count
            }
            if v.read_count < min_read_count {
                min_read_count = v.read_count
            }

            if v.write_count > max_write_count {
                max_write_count = v.write_count
            }
            if v.write_count < min_write_count {
                min_write_count = v.write_count
            }
        }

        for (k, v) in target {
            v.timestamp = (v.timestamp - min_timestamp) / (max_timestamp - min_timestamp);
            v.read_count = (v.read_count - min_read_count) / (max_read_count - min_read_count);
            v.write_count = (v.write_count - min_write_count) / (max_write_count - min_write_count);
        }
    }
}
