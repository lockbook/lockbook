use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::file_metadata::DocumentHmac;
use test_utils::lockbook_server_lib::billing::Nop;
use test_utils::lockbook_server_lib::document_service::DocumentService;
use test_utils::lockbook_server_lib::in_process::{InMemDocuments, InMemoryServer};
use test_utils::lockbook_server_lib::{ServerError, ServerState};
use test_utils::{random_name, test_config};
use tokio::sync::Notify;
use uuid::Uuid;

/// The in-memory document store with a pause point, a stand-in like `Nop` is for Stripe. Once
/// armed, its next upload waits for the test to release it.
#[derive(Clone, Default)]
struct GatedDocuments {
    docs: InMemDocuments,
    armed: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait]
impl DocumentService for GatedDocuments {
    async fn insert<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        // The one armed upload parks here, after the server validated the edit and before it
        // commits the new metadata, so the test can run another device's pull inside that window.
        if self.armed.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.docs.insert(id, hmac, content).await
    }

    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        self.docs.get(id, hmac).await
    }

    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        self.docs.maybe_get(id, hmac).await
    }

    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>> {
        self.docs.delete(id, hmac).await
    }

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        self.docs.exists(id, hmac)
    }

    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf {
        self.docs.get_path(id, hmac)
    }
}

type GatedServer = ServerState<Nop, Nop, Nop, GatedDocuments>;
type Lb = lb_rs::Lb<GatedServer>;

fn gated_server() -> GatedServer {
    let base = InMemoryServer::in_memory();
    ServerState {
        config: base.config,
        index_db: base.index_db,
        stripe_client: base.stripe_client,
        google_play_client: base.google_play_client,
        app_store_client: base.app_store_client,
        document_service: GatedDocuments::default(),
        discord_client: base.discord_client,
        recent_new_account_ips: base.recent_new_account_ips,
    }
}

/// A device that pulls while another device's edit is still uploading still gets the edit on
/// its next sync.
#[tokio::test]
async fn pull_during_content_upload_still_gets_the_edit() {
    let server = gated_server();
    let gate = server.document_service.clone();

    let writer = Lb::in_memory(test_config(), server.clone()).await.unwrap();
    writer
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = writer.create_at_path("note.md").await.unwrap().id;
    writer.write_document(doc, b"before").await.unwrap();
    writer.sync().await.unwrap();

    let reader = Lb::in_memory(test_config(), server.clone()).await.unwrap();
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
    let server = gated_server();
    let gate = server.document_service.clone();

    let first = Lb::in_memory(test_config(), server.clone()).await.unwrap();
    first
        .create_account(&random_name(), "", false)
        .await
        .unwrap();
    let doc = first.create_at_path("note.md").await.unwrap().id;
    first.write_document(doc, b"before").await.unwrap();
    first.sync().await.unwrap();

    let second = Lb::in_memory(test_config(), server.clone()).await.unwrap();
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

    let third = Lb::in_memory(test_config(), server.clone()).await.unwrap();
    third.import_account(&key, None).await.unwrap();
    third.sync().await.unwrap();
    assert_eq!(third.read_document(doc, false).await.unwrap(), b"after");
}
