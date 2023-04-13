use crate::exhaustive_sync::trial::Trial;
use basic_human_duration::ChronoHumanDuration;
use itertools::Itertools;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, thread};
use time::Instant;
use uuid::Uuid;

pub type ThreadID = usize;
pub type TrialID = Uuid;

pub struct Experiment {
    pub start_time: Instant,
    pub pending: Vec<Trial>,
    pub errors: u64,
    pub error_log: File,
    pub done: u64,
    pub running: HashMap<ThreadID, (Instant, TrialID)>,
}

impl Default for Experiment {
    fn default() -> Self {
        let start_time = Instant::now();
        let pending = vec![Trial::default()];
        let running = HashMap::new();
        let errors = 0;
        let done = 0;
        fs::create_dir_all("trials").unwrap();
        let error_log = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open("trials/errors.log")
            .unwrap();

        Experiment { start_time, pending, running, done, errors, error_log }
    }
}

type Continue = bool;

impl Experiment {
    pub fn grab_ready_trial_for_thread(
        thread: ThreadID, experiments: Arc<Mutex<Self>>,
    ) -> (Option<Trial>, Continue) {
        let mut state = experiments.lock().unwrap();
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

    pub fn publish_results(
        thread: ThreadID, experiments: Arc<Mutex<Self>>, result: Trial, mutants: &[Trial],
    ) {
        result.maybe_cleanup(thread);
        let mut state = experiments.lock().unwrap();

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
        let state = Arc::new(Mutex::new(self));

        for thread_id in 0..num_cpus::get() {
            fs::create_dir_all(format!("trials/{}", thread_id)).unwrap();
            let thread_state = state.clone();
            thread::Builder::new()
                .name(format!("{}", thread_id))
                .spawn(move || loop {
                    match Self::grab_ready_trial_for_thread(thread_id, thread_state.clone()) {
                        (Some(mut work), _) => {
                            let mut mutants = work.execute(thread_id);
                            mutants.reverse();
                            Self::publish_results(thread_id, thread_state.clone(), work, &mutants);
                        }
                        (None, true) => {
                            thread::sleep(Duration::from_millis(100));
                        }
                        (None, false) => {
                            println!("no work found, stopping");
                            break;
                        }
                    }
                })
                .unwrap();
        }

        // Info loop
        loop {
            {
                let experiments = state.lock().unwrap();
                if experiments.pending.is_empty()
                    && experiments.running.is_empty()
                    && experiments.done > 0
                {
                    println!("done printing info");
                    break;
                }
                println!(
                    "Done: {}, Errors: {}, TPS: {}, Started: {}, Possibly Stalled: {:?}",
                    experiments.done,
                    experiments.errors,
                    experiments.trials_per_second(),
                    experiments.uptime(),
                    experiments.possibly_stalled()
                );
            }
            thread::sleep(Duration::from_secs(5));
        }
    }

    fn uptime(&self) -> String {
        let duration = self.start_time.elapsed();
        duration.format_human().to_string()
    }

    fn trials_per_second(&self) -> u64 {
        let seconds = self.start_time.elapsed().whole_seconds() as u64;
        let trials = self.done + self.errors;

        if seconds == 0 {
            return 0;
        }

        trials / seconds
    }

    fn possibly_stalled(&self) -> Vec<String> {
        self.running
            .iter()
            .map(|(thread, (start_time, trial_id))| (thread, start_time.elapsed(), trial_id))
            .filter(|(_, elapsed, _)| elapsed.whole_seconds() > 10)
            .sorted_by(|(_, elapsed_a, _), (_, elapsed_b, _)| Ord::cmp(&elapsed_a, &elapsed_b))
            .rev()
            .take(5)
            .map(|(thread, elapsed, trial)| {
                format!("{:?}s, {}/{}", elapsed.whole_seconds(), thread, trial)
            })
            .collect()
    }
}
