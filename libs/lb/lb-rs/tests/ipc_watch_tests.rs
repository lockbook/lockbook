#![cfg(not(any(target_family = "wasm", target_os = "ios")))]

use std::time::Duration;

use db_rs::View;
use lb_rs::Lb;
use lb_rs::service::events::Event;
use test_utils::test_core;
use tokio::time::timeout;

#[tokio::test]
async fn log_watcher_catches_up_without_a_caller_transaction() {
    let writer = test_core().await;
    let mut config = writer.config.clone();
    config.background_work = true;
    let reader = Lb::init(config).await.unwrap();
    // Keep periodic sync from doing the catch-up we're expecting from the watcher.
    let _sync = reader.syncer.lock().await;
    let mut events = reader.subscribe();

    for last_synced in [123, 456] {
        let mut tx = writer.begin_tx().await;
        tx.db().last_synced.replace(last_synced).unwrap();
        tx.end();

        timeout(Duration::from_secs(5), async {
            while !matches!(events.recv().await.unwrap(), Event::IpcChangesApplied) {}
        })
        .await
        .expect("the log watcher did not catch up");
        assert_eq!(reader.ro_tx().await.db().last_synced.as_ref(), Some(&last_synced));

        // The next commit goes into a new log, which the directory watch must also observe.
        writer.db.write().await.snapshot().unwrap();
    }
}
