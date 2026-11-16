use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use lb_rs::io::docs::AsyncDocs;
use test_utils::gate::{DocOp, Gate, GatedDocuments, Point};
use test_utils::lockbook_server_lib::in_process::InMemoryServer;
use test_utils::{random_name, test_config};
use tokio::sync::Notify;

/// Once armed, parks the device's next document read until the test releases it.
#[derive(Clone, Default)]
struct Read {
    armed: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl Gate for Read {
    async fn pass(&self, point: Point) {
        if let Point::Document { op: DocOp::Get, .. } = point {
            if self.armed.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
        }
    }
}

/// While sync merges a document both devices edited, the device can still change its files, and
/// the change survives the merge.
#[tokio::test]
async fn edit_during_merge_is_kept() {
    let server = InMemoryServer::in_memory();
    let other = lb_rs::Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    other
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = other.create_at_path("note.md").await.unwrap().id;
    other.write_document(doc, b"line\n").await.unwrap();
    other.sync().await.unwrap();

    let gate = Read::default();
    let device = lb_rs::Lb::in_memory(
        test_config(),
        server,
        GatedDocuments::new(AsyncDocs::in_memory(), gate.clone()),
    )
    .await
    .unwrap();
    let key = other.export_account_private_key().unwrap();
    device.import_account(&key, None).await.unwrap();
    device.sync().await.unwrap();

    other.write_document(doc, b"line\nother\n").await.unwrap();
    other.sync().await.unwrap();
    device.write_document(doc, b"mine\nline\n").await.unwrap();

    // the merge reads the base, remote, and local versions of the document; park on the first
    gate.armed.store(true, Ordering::SeqCst);
    let edit_mid_merge = async {
        gate.entered.notified().await;
        let created = device.create_at_path("created.md").await.unwrap().id;
        gate.release.notify_one();
        created
    };
    let both = async { tokio::join!(device.sync(), edit_mid_merge) };
    let (synced, created) = tokio::time::timeout(Duration::from_secs(10), both)
        .await
        .expect("an edit doesn't wait for the merge");
    synced.unwrap();

    device.sync().await.unwrap();
    other.sync().await.unwrap();
    assert_eq!(other.get_file_by_id(created).await.unwrap().name, "created.md");
    let merged = String::from_utf8(other.read_document(doc, false).await.unwrap()).unwrap();
    assert!(merged.contains("mine") && merged.contains("other"), "{merged}");
}
