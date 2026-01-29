use super::coordinator::{Coordinator, CoordinatorState};
use basic_human_duration::ChronoHumanDuration;
use itertools::Itertools;
use std::sync::atomic::Ordering;
use std::thread;
use web_time::Duration;

impl Coordinator {
    pub fn print_stats_until_done(&self) {
        loop {
            let cache_size = self.cache.size();
            let experiments = self.state.lock().unwrap();
            if experiments.pending.is_empty()
                && experiments.running.is_empty()
                && experiments.done > 0
            {
                println!("done printing info");
                break;
            }
            println!(
                "Done: {}, Errors: {}, TPS: {}, Started: {}, Possibly Stalled: {:?}, Cache size: {}, grab: {}, work: {}, publish: {}, time_in_locks: {}",
                experiments.done,
                experiments.errors,
                experiments.trials_per_second(),
                experiments.uptime(),
                experiments.possibly_stalled(),
                cache_size,
                self.grab_time.load(Ordering::Relaxed),
                self.execute_time.load(Ordering::Relaxed),
                self.publish_time.load(Ordering::Relaxed),
                self.lock_contention_time.load(Ordering::Relaxed),
            );
            drop(experiments);
            thread::sleep(Duration::from_secs(5));
        }
    }
}

impl CoordinatorState {
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
