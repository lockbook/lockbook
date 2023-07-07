use super::{trial::Trial, trial_cache::TrialCache, worker::Worker};
use std::{
    collections::HashMap,
    fs,
    fs::{File, OpenOptions},
    io::Write,
    sync::{Arc, Mutex},
};
use time::Instant;
use uuid::Uuid;

pub type ThreadID = usize;
pub type TrialID = Uuid;
type Continue = bool;

#[derive(Default, Clone)]
pub struct Coordinator {
    pub state: Arc<Mutex<CoordinatorState>>,

    pub cache: TrialCache,
}

pub struct CoordinatorState {
    pub start_time: Instant,
    pub error_log: File,
    pub pending: Vec<Trial>,
    pub running: HashMap<ThreadID, (Instant, TrialID)>,
    pub errors: u64,
    pub done: u64,
}

impl Default for CoordinatorState {
    fn default() -> Self {
        fs::create_dir_all("trials").unwrap();
        Self {
            error_log: OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open("trials/errors.log")
                .unwrap(),
            pending: vec![Trial::default()],
            running: Default::default(),
            errors: Default::default(),
            done: Default::default(),
            start_time: Instant::now(),
        }
    }
}

impl Coordinator {
    pub fn grab_ready_trial_for_thread(&self, thread: ThreadID) -> (Option<Trial>, Continue) {
        let mut state = self.state.lock().unwrap();
        let experiment = state.pending.pop();
        match experiment {
            Some(found) => {
                found.persist(thread);
                state.running.insert(thread, (Instant::now(), found.id));
                (Some(found), true)
            }
            None => (None, !state.running.is_empty() || !state.pending.is_empty()),
        }
    }

    pub fn publish_results(&self, thread: ThreadID, result: Trial, mutants: &[Trial]) {
        result.maybe_cleanup(thread);
        let mut state = self.state.lock().unwrap();

        if result.failed() {
            writeln!(state.error_log, "{}", result.file_name(thread))
                .unwrap_or_else(|err| eprintln!("failed to write failure to file: {:?}", err));
            state.errors += 1;
        } else {
            state.done += 1;
        }

        state.running.remove(&thread);
        state.pending.extend_from_slice(mutants);
    }

    pub fn kick_off(self) {
        for thread_id in 0..num_cpus::get() {
            Worker::spawn(thread_id, self.clone(), self.cache.clone());
        }

        self.print_stats_until_done();
    }
}
