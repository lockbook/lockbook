use std::sync::atomic::Ordering;
use std::time::Duration;
use std::{fs, thread};

use super::coordinator::{Coordinator, ThreadID};
use super::trial_cache::TrialCache;

pub struct Worker {
    id: ThreadID,
    coord: Coordinator,
    cache: TrialCache,
}

impl Worker {
    pub fn spawn(id: ThreadID, coord: Coordinator, cache: TrialCache) {
        thread::Builder::new()
            .name(format!("{id}"))
            .spawn(move || {
                fs::create_dir_all(format!("trials/{id}")).unwrap();
                Worker { id, coord, cache }.work();
            })
            .unwrap();
    }

    fn work(&self) {
        loop {
            let trial = self.coord.grab_ready_trial_for_thread(self.id);
            match trial {
                (Some(mut work), _) => {
                    let now = web_time::Instant::now();
                    let mut mutants = work.execute(self.id, &self.cache);
                    mutants.reverse();
                    let elapsed = now.elapsed().whole_milliseconds() as u64;
                    self.coord
                        .execute_time
                        .fetch_add(elapsed, Ordering::Relaxed);
                    self.coord.publish_results(self.id, work, &mutants);
                }
                (None, true) => {
                    thread::sleep(Duration::from_millis(100));
                }
                (None, false) => {
                    println!("no work found, stopping");
                    break;
                }
            }
        }
    }
}
