//! Integration tests for the host/guest IPC fallback. When a second `Lb::init`
//! hits the same `writeable_path`, it loses the db-rs lock and should transparently
//! become a `Lb::Remote` that forwards calls to the host over UDS.
//!
//! These tests are unix-only: Windows/WASM don't spawn the listener.

#![cfg(unix)]

use lb_rs::Lb;
use lb_rs::service::events::Event;
use std::time::Duration;
use test_utils::{random_name, test_config, url};
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;

/// Drain the receiver until an event matching `pred` arrives, or time out.
/// Used by subscription tests to tolerate unrelated events (status updates,
/// sync ticks) that may precede the one under test.
async fn await_event<F: Fn(&Event) -> bool>(rx: &mut Receiver<Event>, pred: F) -> Option<Event> {
    timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(evt) if pred(&evt) => return Some(evt),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

#[tokio::test]
async fn solo_init_is_local() {
    let config = test_config();
    let lb = Lb::init(config).await.unwrap();
    assert!(matches!(lb, Lb::Local(_)), "first init with fresh dir should be Local");
}

#[tokio::test]
async fn second_init_becomes_remote() {
    let config = test_config();

    let host = Lb::init(config.clone()).await.unwrap();
    assert!(matches!(host, Lb::Local(_)));

    let guest = Lb::init(config.clone()).await.unwrap();
    assert!(matches!(guest, Lb::Remote(_)), "second init on same path should be Remote");
}

#[tokio::test]
async fn guest_inherits_host_account() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    let account = host
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    assert!(matches!(guest, Lb::Remote(_)));

    // `get_account` on the guest reads from the cache populated at connect().
    let guest_account = guest.get_account().unwrap();
    assert_eq!(guest_account, &account);
}

#[tokio::test]
async fn host_write_visible_to_guest() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let file = host.create_at_path("host.md").await.unwrap();
    host.write_document(file.id, b"host data").await.unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let content = guest.read_document(file.id, false).await.unwrap();
    assert_eq!(content.as_slice(), b"host data");
}

#[tokio::test]
async fn guest_write_visible_to_host() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let file = guest.create_at_path("guest.md").await.unwrap();
    guest.write_document(file.id, b"guest data").await.unwrap();

    let content = host.read_document(file.id, false).await.unwrap();
    assert_eq!(content.as_slice(), b"guest data");
}

#[tokio::test]
async fn two_guests_share_host() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest1 = Lb::init(config.clone()).await.unwrap();
    let guest2 = Lb::init(config.clone()).await.unwrap();
    assert!(matches!(guest1, Lb::Remote(_)));
    assert!(matches!(guest2, Lb::Remote(_)));

    // guest1 creates; guest2 should see the file via the host.
    let file = guest1.create_at_path("shared.md").await.unwrap();
    let seen = guest2.get_file_by_id(file.id).await.unwrap();
    assert_eq!(seen.name, "shared.md");
}

#[tokio::test]
async fn guest_list_metadatas_roundtrip() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    host.create_at_path("a.md").await.unwrap();
    host.create_at_path("b.md").await.unwrap();
    host.create_at_path("subdir/").await.unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let host_metas = host.list_metadatas().await.unwrap();
    let guest_metas = guest.list_metadatas().await.unwrap();
    assert_eq!(host_metas.len(), guest_metas.len());
}

#[tokio::test]
async fn guest_receives_event_from_host() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let mut events = guest.subscribe();

    // `subscribe` spawns the Subscribe request; give it a beat to land on the
    // host before we emit an event.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // `create_at_path` on the host should emit a MetadataChanged event that
    // the server forwards to the guest over Frame::Event.
    host.create_at_path("event-fodder.md").await.unwrap();

    let evt = await_event(&mut events, |e| matches!(e, Event::MetadataChanged(_))).await;
    assert!(evt.is_some(), "guest did not receive MetadataChanged within 5s");
}

#[tokio::test]
async fn guest_receives_document_written_event() {
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();
    let file = host.create_at_path("doc-event.md").await.unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let mut events = guest.subscribe();
    tokio::time::sleep(Duration::from_millis(100)).await;

    host.write_document(file.id, b"payload").await.unwrap();

    let evt =
        await_event(&mut events, |e| matches!(e, Event::DocumentWritten(id, _) if *id == file.id))
            .await;
    assert!(evt.is_some(), "guest did not receive DocumentWritten for {}", file.id);
}

#[tokio::test]
async fn guest_own_write_fires_event_on_guest() {
    // Guest-initiated write: the guest ships the request to the host, the host
    // emits the event, and the event must travel back to the guest's subscriber.
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let mut events = guest.subscribe();
    tokio::time::sleep(Duration::from_millis(100)).await;

    guest.create_at_path("guest-owned.md").await.unwrap();

    let evt = await_event(&mut events, |e| matches!(e, Event::MetadataChanged(_))).await;
    assert!(evt.is_some(), "guest did not receive its own MetadataChanged");
}

#[tokio::test]
async fn multiple_subscribers_on_one_guest() {
    // Both receivers share the single IPC Subscribe (OnceLock-gated), so one
    // host event should fan out to every receiver on the guest.
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let mut rx_a = guest.subscribe();
    let mut rx_b = guest.subscribe();
    tokio::time::sleep(Duration::from_millis(100)).await;

    host.create_at_path("fanout.md").await.unwrap();

    let evt_a = await_event(&mut rx_a, |e| matches!(e, Event::MetadataChanged(_))).await;
    let evt_b = await_event(&mut rx_b, |e| matches!(e, Event::MetadataChanged(_))).await;
    assert!(evt_a.is_some(), "first receiver did not get the event");
    assert!(evt_b.is_some(), "second receiver did not get the event");
}

#[tokio::test]
async fn two_guests_each_get_their_own_stream() {
    // Each guest opens its own Subscribe request; each should see the host's event.
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest1 = Lb::init(config.clone()).await.unwrap();
    let guest2 = Lb::init(config.clone()).await.unwrap();
    let mut rx1 = guest1.subscribe();
    let mut rx2 = guest2.subscribe();
    tokio::time::sleep(Duration::from_millis(100)).await;

    host.create_at_path("per-guest.md").await.unwrap();

    let e1 = await_event(&mut rx1, |e| matches!(e, Event::MetadataChanged(_))).await;
    let e2 = await_event(&mut rx2, |e| matches!(e, Event::MetadataChanged(_))).await;
    assert!(e1.is_some(), "guest1 did not receive event");
    assert!(e2.is_some(), "guest2 did not receive event");
}

#[tokio::test]
async fn guest_receives_sequence_of_events() {
    // A burst of host-side operations: the guest must receive all of them, in
    // enough order to associate them with the right ids.
    let config = test_config();
    let host = Lb::init(config.clone()).await.unwrap();
    host.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let guest = Lb::init(config.clone()).await.unwrap();
    let mut events = guest.subscribe();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let f1 = host.create_at_path("seq-1.md").await.unwrap();
    let f2 = host.create_at_path("seq-2.md").await.unwrap();
    host.write_document(f1.id, b"one").await.unwrap();
    host.write_document(f2.id, b"two").await.unwrap();

    let dw_f1 =
        await_event(&mut events, |e| matches!(e, Event::DocumentWritten(id, _) if *id == f1.id))
            .await;
    let dw_f2 =
        await_event(&mut events, |e| matches!(e, Event::DocumentWritten(id, _) if *id == f2.id))
            .await;
    assert!(dw_f1.is_some(), "missed DocumentWritten for f1");
    assert!(dw_f2.is_some(), "missed DocumentWritten for f2");
}
