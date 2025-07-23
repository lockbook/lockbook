use lb_rs::service::api_service::no_network::CoreIP;

use super::trial::{Action, Trial};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

const CACHE_SIZE: usize = 500000;

#[derive(Default, Clone)]
pub struct TrialCache {
    state: Arc<RwLock<TrialCacheState>>,
}

#[derive(Default)]
struct TrialCacheState {
    trials: HashMap<Vec<Action>, Vec<Vec<CoreIP>>>,
}

impl TrialCache {
    pub fn ready(&self) -> bool {
        !self.state.read().unwrap().trials.is_empty()
    }

    pub fn get(&self, actions: &[Action]) -> (Vec<Vec<CoreIP>>, usize) {
        for i in (0..=actions.len()).rev() {
            let actions = actions[0..i].to_vec();
            if let Some(entry) = self.state.read().unwrap().trials.get(&actions) {
                return (deep_copy(entry), i);
            }
        }

        panic!("cache was ready but no entries found");
    }

    pub fn populate(&self, trial: &Trial) {
        if self.state.read().unwrap().trials.len() >= CACHE_SIZE {
            return;
        }

        let steps = trial.steps[0..trial.completed_steps].to_vec();
        if self.state.read().unwrap().trials.get(&steps).is_some() {
            return;
        }

        self.state
            .write()
            .unwrap()
            .trials
            .insert(steps, deep_copy(&trial.devices_by_user));
    }

    pub fn size(&self) -> usize {
        self.state.read().unwrap().trials.len()
    }
}

// todo: don't needlessly deep copy server for each core
fn deep_copy(incoming: &[Vec<CoreIP>]) -> Vec<Vec<CoreIP>> {
    let mut outgoing = incoming.to_vec();
    let server = incoming[0][0].deep_copy().1;

    for (udx, user) in incoming.iter().enumerate() {
        for (cdx, core) in user.iter().enumerate() {
            outgoing[udx][cdx] = core.deep_copy().0;
            outgoing[udx][cdx].set_client(server.clone());
        }
    }
    outgoing
}
