//! A document store that asks a [`Gate`] before each operation, so a test decides when a
//! request that reached the store goes on.

use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::file_metadata::DocumentHmac;
use lockbook_server_lib::ServerError;
use lockbook_server_lib::document_service::DocumentService;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocOp {
    Insert,
    Get,
    MaybeGet,
    Delete,
}

/// A store operation about to run.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub op: DocOp,
    pub id: Uuid,
}

#[async_trait]
pub trait Gate: Send + Sync + 'static {
    /// The operation runs once this returns.
    async fn pass(&self, point: Point);
}

/// `D` with every async operation passed through `gate` first.
#[derive(Clone)]
pub struct GatedDocuments<D> {
    pub docs: D,
    pub gate: Arc<dyn Gate>,
}

impl<D> GatedDocuments<D> {
    pub fn new(docs: D, gate: impl Gate) -> Self {
        Self { docs, gate: Arc::new(gate) }
    }
}

#[async_trait]
impl<D: DocumentService> DocumentService for GatedDocuments<D> {
    async fn insert<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        self.gate.pass(Point { op: DocOp::Insert, id: *id }).await;
        self.docs.insert(id, hmac, content).await
    }

    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        self.gate.pass(Point { op: DocOp::Get, id: *id }).await;
        self.docs.get(id, hmac).await
    }

    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        self.gate.pass(Point { op: DocOp::MaybeGet, id: *id }).await;
        self.docs.maybe_get(id, hmac).await
    }

    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>> {
        self.gate.pass(Point { op: DocOp::Delete, id: *id }).await;
        self.docs.delete(id, hmac).await
    }

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        self.docs.exists(id, hmac)
    }

    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf {
        self.docs.get_path(id, hmac)
    }
}
