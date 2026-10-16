#![cfg(not(any(target_family = "wasm", target_os = "ios")))]

use lb_rs::{Lb, LbErrKind};
use std::{fs::File, path::Path};
use test_utils::test_core;

#[tokio::test]
async fn sync_lock_excludes_other_syncers_but_not_transactions() {
    let first = test_core().await;
    let second = Lb::init(first.config.clone()).await.unwrap();
    let lock = File::create(Path::new(&first.config.writeable_path).join("sync.lock")).unwrap();
    lock.try_lock().unwrap();

    assert_eq!(second.sync().await.unwrap_err().kind, LbErrKind::AlreadySyncing);
    assert_eq!(second.server_dirty_ids().await.unwrap_err().kind, LbErrKind::AlreadySyncing);
    second.begin_tx().await.end();

    drop(lock);
    assert_eq!(second.sync().await.unwrap_err().kind, LbErrKind::AccountNonexistent);
}

#[tokio::test]
async fn sync_error_releases_the_file_lock() {
    let core = test_core().await;

    assert_eq!(core.sync().await.unwrap_err().kind, LbErrKind::AccountNonexistent);

    let lock = File::options()
        .read(true)
        .write(true)
        .open(Path::new(&core.config.writeable_path).join("sync.lock"))
        .unwrap();
    lock.try_lock().unwrap();
}

#[tokio::test]
async fn sync_catches_up_before_requesting_server_updates() {
    let first = test_core().await;
    let second = Lb::init(first.config.clone()).await.unwrap();
    let mut tx = first.begin_tx().await;
    tx.db().last_synced.replace(123).unwrap();
    tx.end();

    // Without an account, sync stops at its first server request.
    assert_eq!(second.sync().await.unwrap_err().kind, LbErrKind::AccountNonexistent);
    assert_eq!(second.ro_tx().await.db().last_synced.as_ref(), Some(&123));
}
