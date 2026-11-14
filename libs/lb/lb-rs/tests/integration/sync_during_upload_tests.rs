use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use lb_rs::io::docs::AsyncDocs;
use test_utils::gate::{DocOp, Gate, GatedDocuments, Point};
use test_utils::lockbook_server_lib::ServerState;
use test_utils::lockbook_server_lib::billing::Nop;
use test_utils::lockbook_server_lib::in_process::InMemDocuments;
use test_utils::{random_name, test_config};
use tokio::sync::Notify;

/// Once armed, parks the server's next upload until the test releases it: after the server
/// validated the edit and before it commits the new metadata.
#[derive(Clone, Default)]
struct Upload {
    armed: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl Gate for Upload {
    async fn pass(&self, point: Point) {
        if let Point::Document { op: DocOp::Insert, .. } = point {
            if self.armed.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
        }
    }
}

type TestServer = ServerState<Nop, Nop, Nop, GatedDocuments<InMemDocuments>>;
type Lb = lb_rs::Lb<TestServer>;

fn gated_server() -> (TestServer, Upload) {
    let gate = Upload::default();
    let docs = GatedDocuments::new(InMemDocuments::default(), gate.clone());
    (TestServer::with_documents(docs), gate)
}

/// A device that pulls while another device's edit is still uploading still gets the edit on
/// its next sync.
#[tokio::test]
async fn pull_during_content_upload_still_gets_the_edit() {
    let (server, gate) = gated_server();

    let writer = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    writer
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = writer.create_at_path("note.md").await.unwrap().id;
    writer.write_document(doc, b"before").await.unwrap();
    writer.sync().await.unwrap();

    let reader = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    let key = writer.export_account_private_key().unwrap();
    reader.import_account(&key, None).await.unwrap();
    reader.sync().await.unwrap();

    writer.write_document(doc, b"after").await.unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    let pull_mid_upload = async {
        gate.entered.notified().await;
        // the server's clock counts milliseconds; let one pass after the upload began
        tokio::time::sleep(Duration::from_millis(5)).await;
        reader.sync().await.unwrap();
        gate.release.notify_one();
    };
    let both = async { tokio::join!(writer.sync(), pull_mid_upload) };
    let (pushed, ()) = tokio::time::timeout(Duration::from_secs(10), both)
        .await
        .expect("the writer's sync uploads the edit");
    pushed.unwrap();

    reader.sync().await.unwrap();
    assert_eq!(reader.read_document(doc, false).await.unwrap(), b"after");
}

/// Two devices that upload the same edit at once leave the note readable on a third device.
#[tokio::test]
async fn same_edit_uploaded_twice_at_once_stays_readable() {
    let (server, gate) = gated_server();

    let first = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    first
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = first.create_at_path("note.md").await.unwrap().id;
    first.write_document(doc, b"before").await.unwrap();
    first.sync().await.unwrap();

    let second = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    let key = first.export_account_private_key().unwrap();
    second.import_account(&key, None).await.unwrap();
    second.sync().await.unwrap();

    first.write_document(doc, b"after").await.unwrap();
    second.write_document(doc, b"after").await.unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    let second_uploads_mid_upload = async {
        gate.entered.notified().await;
        second.sync().await.unwrap();
        gate.release.notify_one();
    };
    let both = async { tokio::join!(first.sync(), second_uploads_mid_upload) };
    // the parked upload loses the race and is refused
    let (_refused, ()) = tokio::time::timeout(Duration::from_secs(10), both)
        .await
        .expect("both uploads finish");

    let third = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    third.import_account(&key, None).await.unwrap();
    third.sync().await.unwrap();
    assert_eq!(third.read_document(doc, false).await.unwrap(), b"after");
}

/// An edit undone while its upload is still in flight stays undone once the device syncs again.
#[tokio::test]
async fn undo_during_content_upload_keeps_the_undo() {
    let (server, gate) = gated_server();

    let writer = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    writer
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = writer.create_at_path("note.md").await.unwrap().id;
    writer.write_document(doc, b"before").await.unwrap();
    writer.sync().await.unwrap();

    writer.write_document(doc, b"after").await.unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    let undo_mid_upload = async {
        gate.entered.notified().await;
        writer.write_document(doc, b"before").await.unwrap();
        gate.release.notify_one();
    };
    let both = async { tokio::join!(writer.sync(), undo_mid_upload) };
    let (pushed, ()) = tokio::time::timeout(Duration::from_secs(10), both)
        .await
        .expect("the writer's sync uploads the edit");
    pushed.unwrap();

    writer.sync().await.unwrap();
    assert_eq!(writer.read_document(doc, false).await.unwrap(), b"before");

    let reader = Lb::in_memory(test_config(), server.clone(), AsyncDocs::in_memory())
        .await
        .unwrap();
    let key = writer.export_account_private_key().unwrap();
    reader.import_account(&key, None).await.unwrap();
    reader.sync().await.unwrap();
    assert_eq!(reader.read_document(doc, false).await.unwrap(), b"before");
}
