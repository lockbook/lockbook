use std::{fs, thread, time::Duration};

use super::{
    coordinator::{Coordinator, ThreadID},
    trial_cache::TrialCache,
};

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
            match self.coord.grab_ready_trial_for_thread(self.id) {
                (Some(mut work), _) => {
                    let mut mutants = work.execute(self.id, &self.cache);
                    mutants.reverse();
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
