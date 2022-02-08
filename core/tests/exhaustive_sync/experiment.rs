use core::time;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use itertools::Itertools;
use uuid::Uuid;

use lockbook_crypto::clock_service::{get_time, Timestamp};

use crate::exhaustive_sync::trial::{Status, Trial};

pub type ThreadID = usize;

#[derive(Clone)]
pub struct Experiment {
    pub pending: Vec<Trial>,
    pub concluded: Vec<Trial>,
    pub running: HashMap<ThreadID, (Timestamp, Trial)>,
}

impl Default for Experiment {
    fn default() -> Self {
        Experiment {
            pending: vec![Trial {
                id: Uuid::new_v4(),
                clients: vec![],
                target_clients: 2,
                target_steps: 6,
                steps: vec![],
                completed_steps: 0,
                status: Status::Ready,
                start_time: 0,
                end_time: 0,
            }],
            running: HashMap::new(),
            concluded: vec![],
        }
    }
}

type Continue = bool;

impl Experiment {
    pub fn grab_ready_trial_for_thread(
        thread: ThreadID,
        experiments: Arc<Mutex<Self>>,
    ) -> (Option<Trial>, Continue) {
        let mut state = experiments.lock().unwrap();
        let experiment = state.pending.pop();
        match experiment {
            Some(found) => {
                state.running.insert(thread, (get_time(), found.clone()));
                (Some(found), true)
            }
            None => (None, !state.running.is_empty()),
        }
    }

    pub fn publish_results(
        thread: ThreadID,
        experiments: Arc<Mutex<Self>>,
        result: Trial,
        mutants: &[Trial],
    ) {
        let mut state = experiments.lock().unwrap();
        state.running.remove(&thread);
        state.concluded.push(result);
        state.pending.extend_from_slice(mutants);
    }

    pub fn kick_off(self) {
        let state = Arc::new(Mutex::new(self));

        for thread in 0..num_cpus::get() * 2 {
            let thread_state = state.clone();
            thread::spawn(move || loop {
                match Self::grab_ready_trial_for_thread(thread, thread_state.clone()) {
                    (Some(mut work), _) => {
                        let mutants = work.execute();
                        Self::publish_results(thread, thread_state.clone(), work, &mutants);
                    }
                    (None, true) => {
                        thread::sleep(time::Duration::from_millis(100));
                    }
                    (None, false) => break,
                }
            });
        }

        let mut print_count = 0;
        loop {
            print_count += 1;
            thread::sleep(time::Duration::from_millis(10000));
            let experiments = state.lock().unwrap().clone();
            let mut failures = experiments.concluded.clone();
            failures.retain(|trial| trial.status.failed());
            if experiments.pending.is_empty() && experiments.running.is_empty() {
                break;
            }

            let stuck: HashMap<ThreadID, (Timestamp, Trial)> = experiments
                .running
                .clone()
                .into_iter()
                .filter(|(_, (time, _))| time.0 != 0 && get_time().0 - time.0 > 10000)
                .collect();

            println!(
                // show count of trails that have been running over 10 seconds
                "{} pending, {} running, {} stuck, {} run, {} failures.",
                &experiments.pending.len(),
                &experiments.running.len(),
                &stuck.len(),
                &experiments.concluded.len(),
                &failures.len()
            );

            if (!failures.is_empty() || !stuck.is_empty()) && print_count % 12 == 0 {
                println!("failures: {:#?}", failures);
                println!("stuck: {:#?}", stuck);
            }

            if print_count % 12 == 0 {
                if let Some(trial) = experiments
                    .concluded
                    .clone()
                    .into_iter()
                    .sorted_by_key(|t| t.end_time - t.start_time)
                    .last()
                {
                    println!(
                        "slowest trial took {}s: {:#?}",
                        (trial.end_time - trial.start_time) as f64 / 1000.0,
                        trial
                    );
                }
            }
        }

        let experiments = state.lock().unwrap();
        let mut failures = experiments.concluded.clone();
        failures.retain(|trial| trial.status.failed());

        println!(
            "{} trials concluded with {} failures.",
            experiments.concluded.len(),
            failures.len()
        );

        println!("{:#?}", failures);
    }
}
