mod integration_test;

/// Run with: cargo test load_tests -- --nocapture
#[cfg(test)]
mod load_tests {
    use crate::integration_test::{api_url, random_uuid, test_config};
    use std::{thread, time};
    use atomic_counter::{RelaxedCounter, AtomicCounter};
    use std::sync::Arc;
    use std::ops::Add;
    use std::io::Write;
    use indicatif::{ProgressIterator, ProgressBar, ProgressStyle};
    use std::time::Duration;

    #[test]
    fn create_and_sync() {
        let cfg = test_config();
        // let account = lockbook_core::create_account(&cfg, &random_uuid(), &api_url()).unwrap();

        let cpu_count = num_cpus::get();
        println!("Threads: {}", cpu_count);

        let counter = Arc::new(RelaxedCounter::new(0));
        let duration = time::Duration::from_secs(5);
        println!("Spawning {} threads and working for {} seconds", cpu_count, duration.as_secs());

        let mut children = vec![];
        for _ in 0..cpu_count {
            let counter_clone = counter.clone();
            children.push(thread::spawn(move || {
                // Setup
                let end_time = time::Instant::now().add(duration);
                // Let the horses run
                while time::Instant::now() < end_time {
                    // Is Done then break
                    counter_clone.inc();
                }
            }));
        }


        let bar = ProgressBar::new(duration.as_secs());
        bar.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} ({eta}) {msg}")
            .progress_chars("#>-"));
        let start_time = time::Instant::now();
        let end_time = start_time.add(duration);
        while time::Instant::now() < end_time {
            thread::sleep(Duration::from_millis(100));
            let elapsed = (time::Instant::now() - start_time).as_secs();
            if elapsed > 0 {
                bar.set_position(elapsed);
                bar.set_message(format!("{} ops/s", (counter.get() as u64)/elapsed));
            } else {
                bar.set_message(format!("{} ops/s", 0));
            }
        }

        for child in children {
            let _ = child.join();
        }
        bar.finish_with_message(format!("{} ops/s", (counter.get() as u64)/duration.as_secs()));
        bar.finish();

        println!("Completed Operations: {}", counter.get());

        // lockbook_core::create_file_at_path(
        //     &cfg1,
        //     &format!("{}/a/b/c/d/test.txt", account1.username),
        // )
        // .unwrap();
        // lockbook_core::sync_all(&cfg1, None).unwrap();

    }
}
