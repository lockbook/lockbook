//! The syncer's background workers, which every UI client runs (`background_work: true`), read
//! lb's broadcast channel, which holds 16384 events (tokio rounds the 10000 asked for up to a
//! power of two). A document a sync pushes, pulls or prefetches sends two events, its
//! `PushingDocument`/`PullingDocument` pair, and the status worker answers each with a
//! `StatusUpdated`: four per document. A worker that is busy for 4097 documents comes back to
//! an overflowed channel: `recv` reports `Lagged`.
//!
//! Ignored by default for their runtime: `cargo test -p lb-rs --test integration
//! sync_worker_tests -- --ignored`.

use std::time::{Duration, Instant};

use lb_rs::Uuid;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::tree_like::TreeLike;
use test_utils::*;
use tokio::time::sleep;

/// More notes than the channel holds events for: 4500 × 4 = 18000. The import test's own push
/// events pass 16384 on their own; the sign-in test's prefetch does so with the status worker's
/// replies, so if those ever coalesce, raise this past 8192. Pushing a vault through the
/// in-process server costs about the square of its size, so this stays near the line.
const NOTES: usize = 4500;

/// A client as the desktop, iOS and Android apps make one.
async fn ui_core() -> Lb {
    let mut config = test_config();
    config.background_work = true;
    test_core_with(config).await
}

/// Writes `count` notes in folders of 100 under the root and returns their ids.
async fn write_notes(lb: &Lb, count: usize) -> Vec<Uuid> {
    let root = lb.root().await.unwrap();
    let mut ids = Vec::with_capacity(count);
    let mut folder = root.id;
    for i in 0..count {
        if i % 100 == 0 {
            folder = lb
                .create_file(&format!("notes-{}", i / 100), &root.id, FileType::Folder)
                .await
                .unwrap()
                .id;
        }
        let file = lb
            .create_file(&format!("note-{i}.md"), &folder, FileType::Document)
            .await
            .unwrap();
        lb.write_document(file.id, format!("note {i}").as_bytes())
            .await
            .unwrap();
        ids.push(file.id);
    }
    ids
}

/// Whether this client holds the server's version of the document, without fetching it.
async fn holds(lb: &Lb, id: Uuid) -> bool {
    let hmac = {
        let tx = lb.ro_tx().await;
        tx.db()
            .base_metadata
            .maybe_find(&id)
            .and_then(|file| file.document_hmac().copied())
    };
    hmac.is_some() && lb.docs.exists(id, hmac)
}

/// Polls `condition` until it holds or `timeout` passes.
async fn within<F: AsyncFnMut() -> bool>(timeout: Duration, mut condition: F) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if condition().await {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

/// Signing in on a new device prefetches every note so it opens offline. The post-sync worker
/// ran that prefetch; it must still be there for the notes written after it.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "minutes: pushes a 4500-note vault through the in-process server"]
async fn notes_written_after_signing_in_to_a_large_vault_are_still_prefetched() {
    let laptop = test_core_with_account().await;
    let notes = write_notes(&laptop, NOTES).await;
    laptop.sync().await.unwrap();

    let phone = ui_core().await;
    phone
        .import_account(&laptop.export_account_private_key().unwrap(), Some(&url()))
        .await
        .unwrap();
    phone.sync().await.unwrap();
    assert!(
        within(Duration::from_secs(600), async || {
            for id in &notes {
                if !holds(&phone, *id).await {
                    return false;
                }
            }
            true
        })
        .await,
        "the prefetch after sign-in did not finish"
    );

    let root = laptop.root().await.unwrap();
    let late = laptop
        .create_file("late.md", &root.id, FileType::Document)
        .await
        .unwrap();
    laptop
        .write_document(late.id, b"written after the phone signed in")
        .await
        .unwrap();
    laptop.sync().await.unwrap();
    phone.sync().await.unwrap();
    assert!(
        within(Duration::from_secs(60), async || holds(&phone, late.id).await).await,
        "the phone never prefetched the note the laptop wrote after sign-in"
    );
}

/// The user's own edits sync through the local-change worker; once the user has been away for
/// 15 s, the periodic worker syncs only every five minutes. A folder of notes imported while a
/// sync is already running (the periodic worker's, or one the UI asked for) queues the
/// local-change worker behind that sync, and every note imported or pushed meanwhile sends
/// events it reads only once its own sync is done.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "minutes: pushes a 4500-note vault through the in-process server"]
async fn edits_after_a_large_import_during_a_sync_still_sync() {
    let desktop = ui_core().await;
    desktop
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    // The user has been away: the periodic worker is on its five-minute cadence.
    *desktop.user_last_seen.write().await = Instant::now() - Duration::from_secs(60);
    // A quiet moment: the local-change worker drains what sign-in sent and waits for the next event.
    sleep(Duration::from_millis(1500)).await;

    let running_sync = desktop.syncer.lock().await;
    let notes = write_notes(&desktop, NOTES).await;
    drop(running_sync);
    assert!(
        within(Duration::from_secs(600), async || desktop.local_changes().await.is_empty()).await,
        "the import was never pushed"
    );
    // The periodic worker's one sync from before the user left has started by now; let it end.
    sleep(Duration::from_secs(5)).await;
    assert!(
        within(Duration::from_secs(60), async || desktop.syncer.try_lock().is_ok()).await,
        "a sync is still running"
    );

    desktop
        .write_document(notes[0], b"edited after the import")
        .await
        .unwrap();
    assert!(
        within(Duration::from_secs(60), async || desktop.local_changes().await.is_empty()).await,
        "an edit after the import never synced"
    );
}
