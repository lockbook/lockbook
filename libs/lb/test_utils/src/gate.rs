//! Document stores and a client's server that ask a [`Gate`] at each operation and request, so
//! a test decides when a sync or a save goes on.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lb_rs::io::docs::{DocumentStore, LocalDocuments, PendingDocument};
use lb_rs::io::server::{ApiResult, Server};
use lb_rs::model::account::Account;
use lb_rs::model::api::*;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file_metadata::DocumentHmac;
use lockbook_server_lib::document_service::DocumentService;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocOp {
    Insert,
    /// A client's write, before it is promoted.
    InsertPending,
    Get,
    MaybeGet,
    Delete,
}

/// Where a request is.
#[derive(Clone, Copy, Debug)]
pub enum Point {
    /// A store operation about to run, on the server or a client.
    Document { op: DocOp, id: Uuid },
    /// A request about to leave the client.
    Sent { endpoint: &'static str },
    /// A response the server sent, before the client reads it.
    Answered { endpoint: &'static str },
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

    async fn pass(&self, op: DocOp, id: Uuid) {
        self.gate.pass(Point::Document { op, id }).await;
    }
}

impl<D: DocumentStore> DocumentStore for GatedDocuments<D> {
    async fn insert(
        &self, id: Uuid, hmac: DocumentHmac, document: &EncryptedDocument,
    ) -> LbResult<()> {
        self.pass(DocOp::Insert, id).await;
        self.docs.insert(id, hmac, document).await
    }

    async fn get(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<EncryptedDocument> {
        self.pass(DocOp::Get, id).await;
        self.docs.get(id, hmac).await
    }

    async fn maybe_get(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<Option<EncryptedDocument>> {
        self.pass(DocOp::MaybeGet, id).await;
        self.docs.maybe_get(id, hmac).await
    }

    async fn delete(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<()> {
        self.pass(DocOp::Delete, id).await;
        self.docs.delete(id, hmac).await
    }

    fn exists(&self, id: Uuid, hmac: DocumentHmac) -> bool {
        self.docs.exists(id, hmac)
    }
}

impl<D: DocumentService> DocumentService for GatedDocuments<D> {
    fn get_path(&self, id: Uuid, hmac: DocumentHmac) -> PathBuf {
        self.docs.get_path(id, hmac)
    }
}

impl<D: LocalDocuments> LocalDocuments for GatedDocuments<D> {
    async fn insert_pending(
        &self, id: Uuid, hmac: DocumentHmac, document: &EncryptedDocument,
    ) -> LbResult<PendingDocument> {
        self.pass(DocOp::InsertPending, id).await;
        self.docs.insert_pending(id, hmac, document).await
    }

    async fn retain(&self, file_hmacs: HashSet<(Uuid, DocumentHmac)>) -> LbResult<()> {
        self.docs.retain(file_hmacs).await
    }
}

/// `S` with every request passed through `gate` as it leaves and as its answer comes back. The
/// server holds no lock at either point.
#[derive(Clone)]
pub struct GatedServer<S> {
    pub server: S,
    pub gate: Arc<dyn Gate>,
}

impl<S> GatedServer<S> {
    pub fn new(server: S, gate: impl Gate) -> Self {
        Self { server, gate: Arc::new(gate) }
    }
}

macro_rules! gated {
    ($($endpoint:ident: $request:ty),* $(,)?) => {
        $(
            async fn $endpoint(&self, account: &Account, request: $request) -> ApiResult<$request> {
                let endpoint = stringify!($endpoint);
                self.gate.pass(Point::Sent { endpoint }).await;
                let answer = self.server.$endpoint(account, request).await;
                self.gate.pass(Point::Answered { endpoint }).await;
                answer
            }
        )*
    };
}

impl<S: Server> Server for GatedServer<S> {
    gated! {
        new_account_v2: NewAccountRequestV2,
        change_doc_v2: ChangeDocRequestV2,
        upsert_file_metadata_v2: UpsertRequestV2,
        get_document: GetDocRequest,
        get_public_key: GetPublicKeyRequest,
        get_username: GetUsernameRequest,
        get_usage: GetUsageRequest,
        get_file_ids: GetFileIdsRequest,
        get_updates_v2: GetUpdatesRequestV2,
        get_build_info: GetBuildInfoRequest,
        delete_account: DeleteAccountRequest,
        upgrade_account_stripe: UpgradeAccountStripeRequest,
        upgrade_account_google_play: UpgradeAccountGooglePlayRequest,
        upgrade_account_app_store: UpgradeAccountAppStoreRequest,
        cancel_subscription: CancelSubscriptionRequest,
        get_subscription_info: GetSubscriptionInfoRequest,
        get_subscription_info_v2: GetSubscriptionInfoRequestV2,
        upsert_debug_info: UpsertDebugInfoRequest,
        admin_disappear_account: AdminDisappearAccountRequest,
        admin_disappear_file: AdminDisappearFileRequest,
        admin_validate_account: AdminValidateAccountRequest,
        admin_validate_server: AdminValidateServerRequest,
        admin_list_users: AdminListUsersRequest,
        admin_get_account_info: AdminGetAccountInfoRequest,
        admin_file_info: AdminFileInfoRequest,
        admin_set_user_tier: AdminSetUserTierRequest,
        admin_rebuild_index: AdminRebuildIndexRequest,
    }
}
