#![cfg(not(target_family = "wasm"))]

use lb_rs::Lb;
use lb_rs::service::events::{Actor, Event};
use test_utils::{random_name, test_core, test_core_with_account, url};

#[tokio::test]
async fn catch_up_emits_an_ipc_event_for_document_changes() {
    let writer = test_core_with_account().await;
    let id = writer.create_at_path("doc.md").await.unwrap().id;
    let reader = Lb::init(writer.config.clone()).await.unwrap();
    let mut events = reader.subscribe();

    writer.write_document(id, b"new").await.unwrap();
    reader.begin_tx().await.end();

    assert!(matches!(events.try_recv().unwrap(), Event::IpcChangesApplied));
    assert_eq!(reader.read_document(id, false).await.unwrap(), b"new");

    reader.begin_tx().await.end();
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(
            event,
            Event::IpcChangesApplied | Event::MetadataChanged(_) | Event::PendingSharesChanged
        ));
    }
}

#[tokio::test]
async fn catch_up_populates_the_account_keychain_before_returning() {
    let reader = test_core().await;
    let writer = Lb::init(reader.config.clone()).await.unwrap();
    let mut events = reader.subscribe();
    let account = writer
        .create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    let tx = reader.begin_tx().await;
    assert_eq!(reader.get_account().unwrap(), &account);
    assert_eq!(reader.keychain.get_pk().unwrap(), account.public_key());
    tx.end();
    assert!(matches!(events.try_recv().unwrap(), Event::UserSignedIn));
}

#[tokio::test]
async fn reading_in_another_instance_does_not_trigger_reload_notifications() {
    let writer = test_core_with_account().await;
    let id = writer.create_at_path("doc.md").await.unwrap().id;
    writer.write_document(id, b"content").await.unwrap();
    let reader = Lb::init(writer.config.clone()).await.unwrap();
    let mut events = reader.subscribe();

    writer.read_document(id, true).await.unwrap();
    reader.begin_tx().await.end();

    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, Event::IpcChangesApplied));
    }
}

#[tokio::test]
async fn local_writes_keep_their_existing_notifications() {
    let core = test_core_with_account().await;
    let mut events = core.subscribe();

    core.create_at_path("doc.md").await.unwrap();

    assert!(matches!(events.try_recv().unwrap(), Event::MetadataChanged(Actor::User(None))));
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, Event::IpcChangesApplied));
    }
}
