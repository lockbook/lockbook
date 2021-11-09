use crate::exhaustive_sync::trial::Status::Failed;
use crate::exhaustive_sync::trial::{Status, Trial};
use core::time;
use lockbook_crypto::clock_service::{get_time, Timestamp};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

pub struct Experiment {
    pub pending: Vec<Trial>,
    pub running: Vec<Trial>,
    pub concluded: Vec<Trial>,
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
            }],
            running: vec![],
            concluded: vec![],
        }
    }
}

type Continue = bool;

impl Experiment {
    pub fn grab_ready_trial(experiments: Arc<Mutex<Self>>) -> (Option<Trial>, Continue) {
        let mut state = experiments.lock().unwrap();
        let experiment = state.pending.pop();
        match experiment {
            Some(found) => {
                state.running.push(found.clone());
                (Some(found), true)
            }
            None => (None, !state.running.is_empty()),
        }
    }

    pub fn publish_results(experiments: Arc<Mutex<Self>>, result: Trial, mutants: &[Trial]) {
        let mut state = experiments.lock().unwrap();
        state.running.retain(|trial| trial.id != result.id);
        state.concluded.push(result);
        state.pending.extend_from_slice(mutants);
    }

    pub fn kick_off(self) {
        let state = Arc::new(Mutex::new(self));
        let checkins = Arc::new(Mutex::new(HashMap::<Uuid, Timestamp>::new()));

        for _ in 0..num_cpus::get() {
            let thread_state = state.clone();
            let thread_checkins = checkins.clone();
            thread::spawn(move || loop {
                match Self::grab_ready_trial(thread_state.clone()) {
                    (Some(mut work), _) => {
                        thread_checkins.lock().unwrap().insert(work.id, get_time());

                        let mutants = work.execute();
                        thread_checkins.lock().unwrap().remove(&work.id);

                        Self::publish_results(thread_state.clone(), work, &mutants);
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
            let mut experiments = state.lock().unwrap();
            let mut failures = experiments.concluded.clone();
            failures.retain(|trial| trial.status.failed());
            if experiments.pending.is_empty() && experiments.running.is_empty() {
                break;
            }

            for running_trial in &mut experiments.running.clone() {
                if let Some(time) = checkins.lock().unwrap().get(&running_trial.id) {
                    if get_time().0 - time.0 > 30000 {
                        running_trial.status = Failed("Timeout.".to_string());
                        experiments
                            .running
                            .retain(|trial| trial.id != running_trial.id);
                        experiments.concluded.push(running_trial.clone());
                    }
                }
            }

            println!(
                "{} pending, {} running, {} run, {} failures.",
                &experiments.pending.len(),
                &experiments.running.len(),
                &experiments.concluded.len(),
                &failures.len()
            );

            if print_count % 6 == 0 {
                println!("{:#?}", failures);
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
