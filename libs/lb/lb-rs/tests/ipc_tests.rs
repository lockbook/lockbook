#![cfg(unix)]

use lb_rs::service::events::Event;
use lb_rs::Lb;
use std::time::Duration;
use test_utils::{random_name, test_config, url};
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;

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
    assert!(lb.is_local(), "first init with fresh dir should be Local");
}

#[tokio::test]
async fn second_init_becomes_remote() {
    let config = test_config();

    let host = Lb::init(config.clone()).await.unwrap();
    assert!(host.is_local());

    let guest = Lb::init(config.clone()).await.unwrap();
    assert!(!guest.is_local(), "second init on same path should be Remote");
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
    assert!(!guest.is_local());

    let guest_account = guest.get_account().unwrap();
    assert_eq!(guest_account, account);
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
    assert!(!guest1.is_local());
    assert!(!guest2.is_local());

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

    tokio::time::sleep(Duration::from_millis(100)).await;

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

#[tokio::test]
async fn guest_call_recovers_when_host_dies() {
    let config = test_config();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

    let host_config = config.clone();
    let host_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let host = rt.block_on(async {
            let h = Lb::init(host_config).await.unwrap();
            h.create_account(&random_name(), &url(), false)
                .await
                .unwrap();
            h
        });

        ready_tx.send(()).unwrap();
        shutdown_rx.recv().unwrap();

        drop(host);
        drop(rt);
    });

    ready_rx.recv().unwrap();

    let guest = Lb::init(config).await.unwrap();
    assert!(!guest.is_local());
    guest.list_metadatas().await.unwrap();

    shutdown_tx.send(()).unwrap();
    host_thread.join().unwrap();

    let metas = timeout(Duration::from_secs(5), guest.list_metadatas())
        .await
        .expect("guest call hung after host death")
        .expect("guest call should auto-recover after host death");
    assert!(!metas.is_empty(), "expected at least the root file");
    assert!(guest.is_local(), "guest should have promoted itself to Local during recovery");
}

#[tokio::test]
async fn create_file_recovers_after_host_death() {
    let config = test_config();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

    let host_config = config.clone();
    let host_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let host = rt.block_on(async {
            let h = Lb::init(host_config).await.unwrap();
            h.create_account(&random_name(), &url(), false)
                .await
                .unwrap();
            h
        });
        ready_tx.send(()).unwrap();
        shutdown_rx.recv().unwrap();
        drop(host);
        drop(rt);
    });

    ready_rx.recv().unwrap();

    let guest = Lb::init(config).await.unwrap();
    assert!(!guest.is_local());

    let root = guest.root().await.unwrap();
    let alive = guest
        .create_file("alive.md", &root.id, lb_rs::model::file_metadata::FileType::Document)
        .await
        .unwrap();
    assert_eq!(alive.name, "alive.md");

    shutdown_tx.send(()).unwrap();
    host_thread.join().unwrap();

    let f = guest
        .create_file(
            "post-recovery.md",
            &root.id,
            lb_rs::model::file_metadata::FileType::Document,
        )
        .await
        .expect("create_file should succeed after recovery");
    assert_eq!(f.name, "post-recovery.md");
    assert!(
        guest.is_local(),
        "guest should have promoted itself to Local during recovery"
    );
}
