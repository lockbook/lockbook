use std::collections::HashMap;

use crate::Requester;
use crate::{CoreResult, RequestContext};
use itertools::Itertools;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn most_active_documents(&mut self) -> CoreResult<Vec<(Uuid, usize)>> {
        let mut result: HashMap<Uuid, usize> = HashMap::new();

        for (k, v) in self.tx.read_activity.get_all() {
            result.insert(*k, result.get(k).unwrap_or(&0) + *v);
        }
        for (k, v) in self.tx.write_activity.get_all() {
            result.insert(*k, result.get(k).unwrap_or(&0) + *v);
        }

        let result = result
            .into_iter()
            .sorted_by(|a, b| a.1.cmp(&b.1))
            .collect_vec();
        Ok(result)
    }
}
