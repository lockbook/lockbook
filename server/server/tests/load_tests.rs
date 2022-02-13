mod integration_test;

/// Run with: cargo test load_tests -- --nocapture --ignored
#[cfg(test)]
mod load_tests {
    use crate::integration_test::{api_url, random_uuid, test_config};
    use atomic_counter::{AtomicCounter, RelaxedCounter};
    use indicatif::{ProgressBar, ProgressStyle};
    use std::ops::Add;
    use std::sync::Arc;
    use std::time::Duration;
    use std::{thread, time};

    #[test]
    #[ignore]
    fn create_and_sync() {
        let cpu_count = num_cpus::get();
        println!("Threads: {}", cpu_count);

        let counter = Arc::new(RelaxedCounter::new(0));
        let duration = time::Duration::from_secs(60);
        println!("Spawning {} threads and working for {} seconds", cpu_count, duration.as_secs());

        let mut children = vec![];
        for _ in 0..cpu_count {
            let counter_clone = counter.clone();
            children.push(thread::spawn(move || {
                // Setup
                let cfg = test_config();
                lockbook_core::create_account(
                    &cfg,
                    format!("loadtest{}", &random_uuid()).as_str(),
                    &api_url(),
                )
                .expect("Could not create account!");
                lockbook_core::sync_all(&cfg, None).expect("Could not sync!");
                let end_time = time::Instant::now().add(duration);
                // let root = lockbook_core::get_root(&cfg).expect("Could not get root!");
                // let root_id = root.id;
                // let file = lockbook_core::create_file(&cfg, random_uuid().as_str(), root_id, FileType::Document).expect("Could not create file!");
                // Let the horses run
                while time::Instant::now() < end_time {
                    // lockbook_core::write_document(&cfg, file.id, random_uuid().as_bytes()).unwrap();
                    // lockbook_core::sync_all(&cfg, None).unwrap();
                    lockbook_core::calculate_work(&cfg).unwrap();
                    counter_clone.inc();
                }
            }));
        }

        let bar = ProgressBar::new(duration.as_secs());
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} ({eta}) {msg}")
                .progress_chars("#>-"),
        );
        let start_time = time::Instant::now();
        let end_time = start_time.add(duration);
        while time::Instant::now() < end_time {
            thread::sleep(Duration::from_millis(100));
            let elapsed = (time::Instant::now() - start_time).as_secs();
            if elapsed > 0 {
                bar.set_position(elapsed);
                bar.set_message(format!("{} ops/s", (counter.get() as u64) / elapsed));
            } else {
                bar.set_message(format!("{} ops/s", 0));
            }
        }

        for child in children {
            let _ = child.join();
        }
        bar.finish_with_message(format!("{} ops/s", (counter.get() as u64) / duration.as_secs()));
        bar.finish();

        println!("Completed Operations: {}", counter.get());
    }
}
