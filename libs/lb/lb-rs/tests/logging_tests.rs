use lb_rs::model::core_config::Config;
use lb_rs::service::logging;
use std::sync::Barrier;

#[test]
fn logging_initialization_is_safe_for_concurrent_cores() {
    let directory = tempfile::tempdir().unwrap();
    let mut config = Config::cli_config("cli");
    config.writeable_path = directory.path().to_str().unwrap().to_owned();
    config.colored_logs = false;
    config.logs = false;
    logging::init_for_client(&config, Some("disabled")).unwrap();
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    config.logs = true;

    let barrier = Barrier::new(16);
    std::thread::scope(|scope| {
        for worker in 0..16 {
            let (config, barrier) = (&config, &barrier);
            scope.spawn(move || {
                barrier.wait();
                logging::init_for_client(config, Some("file-provider")).unwrap();
                tracing::info!(target: "lb_rs", "[worker {worker}]");
            });
        }
    });
    logging::log_startup("bundle=app.lockbook.FileProvider version=test build=123 commit=abc123");
    // Lb::init calls the unnamed initializer after the Swift bridge initializes logging.
    logging::init(&config).unwrap();
    let contents =
        std::fs::read_to_string(directory.path().join("lockbook-file-provider.log")).unwrap();
    for worker in 0..16 {
        assert!(contents.contains(&format!("[worker {worker}]")));
    }
    assert!(
        contents.contains(
            "startup bundle=app.lockbook.FileProvider version=test build=123 commit=abc123"
        )
    );
    assert!(contents.contains(&format!("pid={}", std::process::id())));
    assert!(contents.contains(lb_rs::get_code_version()));
    assert!(!directory.path().join("lockbook.log").exists());
}
