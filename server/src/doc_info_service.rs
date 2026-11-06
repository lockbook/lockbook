use std::collections::HashSet;

use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::api::{
    GetDocErrorV3, GetDocRequestV3, GetDocResponseV3, GetDocUpdatesError, GetDocUpdatesRequest,
    GetDocUpdatesResponse, PayloadKind, PushDocError, PushDocRequest, PushDocResponse,
};
use lb_rs::model::clock::get_time;
use lb_rs::model::doc_info::{BlobKey, DocInfo, DocVersion, Storage, VersionKind};
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{DocumentHmac, Owner};
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::tree_like::TreeLike;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::ServerError::ClientError;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::defense::SERVER_BANDWIDTH_CAP;
use crate::document_service::DocumentService;
use crate::guard::ServerTx;
use crate::schema::{ServerV6, doc_size};
use crate::{RequestContext, ServerError, ServerState};

pub fn forget_doc(
    db: &mut ServerV6, id: Uuid, legacy_hmac: Option<DocumentHmac>,
) -> Result<(), db_rs::errors::Error> {
    let now = get_time().0;
    match db.doc_info.remove(&id)? {
        Some(info) => {
            for key in info.blobs(id) {
                db.scheduled_blob_cleanups.insert(key, now)?;
            }
        }
        None => {
            if let Some(hmac) = legacy_hmac {
                db.scheduled_blob_cleanups
                    .insert(BlobKey::Legacy { id, hmac }, now)?;
            }
        }
    }
    Ok(())
}

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn push_doc(
        &self, context: RequestContext<PushDocRequest>,
    ) -> Result<PushDocResponse, ServerError<PushDocError>> {
        let requester = Owner(context.public_key);
        let request = context.request;
        if request.kind == PayloadKind::Diff && request.base.is_none() {
            return Err(ClientError(PushDocError::DiffMalformed));
        }
        let id = request.id;
        let compressed_size = request.content.value.len() as u64;
        let version = Uuid::new_v4();
        let key = BlobKey::Version { id, version };

        {
            let mut lock = self.index_db.lock().await;
            let mut tx = ServerTx::begin(&mut lock)?;
            self.check_push(&mut tx, requester, &request, compressed_size)?;
            tx.end()?;
        }

        self.document_service.insert(&key, &request.content).await?;

        let result = async {
            let mut lock = self.index_db.lock().await;
            let mut tx = ServerTx::begin(&mut lock)?;
            let db = &mut *tx;
            self.check_push(db, requester, &request, compressed_size)?;

            let mut info = db.doc_info.get(&id).cloned().unwrap_or_default();
            let now = get_time().0 as u64;
            let timestamp = info
                .head()
                .map(|head| head.timestamp + 1)
                .unwrap_or_default()
                .max(now);
            let kind = match (request.kind, request.base) {
                (PayloadKind::Diff, Some(base)) => VersionKind::Diff { base },
                _ => VersionKind::Snapshot,
            };
            if kind == VersionKind::Snapshot {
                for old_key in info.retire(id) {
                    db.scheduled_blob_cleanups.insert(old_key, now as i64)?;
                }
            }
            info.versions.push(DocVersion {
                hmac: request.hmac,
                kind,
                storage: Storage::Version(version),
                compressed_size,
                uncompressed_size: Some(request.uncompressed_size),
                content_size: Some(request.content_size),
                author: requester,
                timestamp,
            });
            info.updated_at = timestamp;
            let head = info
                .summary()
                .ok_or_else(|| internal!("pushed document has no head"))?;
            db.doc_info.insert(id, info)?;

            if let Some(meta) = db.metas.get(&id) {
                if !meta.file.timestamped_value.value.is_v3() {
                    let mut meta = meta.clone();
                    meta.file.timestamped_value.value = meta.file.timestamped_value.value.into_v3();
                    meta.version = now;
                    db.metas.insert(id, meta)?;
                }
            }

            tx.end()?;
            Ok(PushDocResponse { head })
        }
        .await;

        if result.is_err() {
            if let Err(err) = self.document_service.delete::<PushDocError>(&key).await {
                error!(?err, ?key, "could not clean up a rejected document push");
            }
        }

        result
    }

    fn check_push(
        &self, db: &mut ServerV6, requester: Owner, request: &PushDocRequest, compressed_size: u64,
    ) -> Result<(), ServerError<PushDocError>> {
        use PushDocError::*;
        let id = request.id;

        let owner = db
            .metas
            .get(&id)
            .ok_or(ClientError(DocumentNotFound))?
            .owner();

        let mut tree = ServerTree::new(
            requester,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();
        let is_document = tree
            .maybe_find(&id)
            .ok_or(ClientError(NotPermissioned))?
            .is_document();
        if !is_document {
            return Err(ClientError(NotADocument));
        }
        if tree.calculate_deleted(&id)? {
            return Err(ClientError(DocumentDeleted));
        }
        if tree.access_mode(requester, &id)? < Some(UserAccessMode::Write) {
            return Err(ClientError(NotPermissioned));
        }
        drop(tree);

        let info = db.doc_info.get(&id).cloned().unwrap_or_default();
        if info.head_hmac() != request.base {
            return Err(ClientError(Conflict));
        }
        if request.kind == PayloadKind::Diff
            && info.diff_size() + compressed_size > info.snapshot_size()
        {
            return Err(ClientError(SnapshotRequired));
        }

        let usage_cap = Self::get_cap(db, &owner.0).map_err(|err| internal!("{:?}", err))?;
        let doc_info = &db.doc_info;
        let sizes = |id: &Uuid| doc_size(doc_info, id);
        let mut tree = ServerTree::new(
            owner,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();
        let old_usage = tree.calculate_usage(owner, sizes)?;
        let replaced = match request.kind {
            PayloadKind::Snapshot => info.stored_size(),
            PayloadKind::Diff => 0,
        };
        let new_usage = old_usage.saturating_sub(replaced) + compressed_size;
        debug!(?old_usage, ?new_usage, ?usage_cap, "usage caps on push doc");
        if new_usage > usage_cap && new_usage >= old_usage {
            warn!("user over cap");
            return Err(ClientError(UsageIsOverDataCap));
        }

        Ok(())
    }

    pub async fn get_doc_v3(
        &self, context: RequestContext<GetDocRequestV3>,
    ) -> Result<GetDocResponseV3, ServerError<GetDocErrorV3>> {
        let request = &context.request;
        let requester = Owner(context.public_key);
        let id = request.id;

        let (head, path) = {
            let mut lock = self.index_db.lock().await;
            let mut tx = ServerTx::begin(&mut lock)?;
            let db = &mut *tx;

            let meta_exists = db.metas.get(&id).is_some();
            let mut tree = ServerTree::new(
                requester,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();
            if tree.maybe_find(&id).is_none() {
                return Err(ClientError(if meta_exists {
                    GetDocErrorV3::NotPermissioned
                } else {
                    GetDocErrorV3::DocumentNotFound
                }));
            }
            if tree.calculate_deleted(&id)? {
                return Err(ClientError(GetDocErrorV3::DocumentNotFound));
            }
            drop(tree);

            let info = db.doc_info.get(&id).cloned().unwrap_or_default();
            let path = match info.head() {
                None => vec![],
                Some(_) => info
                    .path(request.have.as_ref(), request.want.as_ref())
                    .ok_or(ClientError(GetDocErrorV3::VersionNotFound))?
                    .to_vec(),
            };
            tx.end()?;
            (info.summary(), path)
        };

        let mut versions = Vec::with_capacity(path.len());
        for version in path {
            let content = self
                .document_service
                .maybe_get(&version.blob(id))
                .await?
                .ok_or(ClientError(GetDocErrorV3::VersionNotFound))?;
            versions.push((version, content));
        }

        if self.config.features.bandwidth_controls {
            let mut lock = self.index_db.lock().await;
            let mut tx = ServerTx::begin(&mut lock)?;
            let db = &mut *tx;

            let mut server_wide = db.server_egress.as_ref().cloned().unwrap_or_default();
            let mut account_bandwidth = db
                .egress_by_owner
                .get(&requester)
                .cloned()
                .unwrap_or_default();
            let account_bandwidth_cap = db
                .accounts
                .get(&requester)
                .map(|account| account.billing_info.bandwidth_cap())
                .unwrap_or_default();

            let size: usize = versions
                .iter()
                .map(|(_, content)| content.value.len())
                .sum();

            if size + server_wide.current_bandwidth() > SERVER_BANDWIDTH_CAP {
                error!("Bandwidth caps are now being enforced");
                if size + account_bandwidth.current_bandwidth() > account_bandwidth_cap {
                    error!("User bandwidth cap exceeded");
                    return Err(ClientError(GetDocErrorV3::BandwidthExceeded));
                }
            }

            server_wide.increase_by(size);
            account_bandwidth.increase_by(size);

            db.server_egress.replace(server_wide)?;
            db.egress_by_owner.insert(requester, account_bandwidth)?;
            tx.end()?;
        }

        Ok(GetDocResponseV3 { head, versions })
    }

    pub async fn get_doc_updates(
        &self, context: RequestContext<GetDocUpdatesRequest>,
    ) -> Result<GetDocUpdatesResponse, ServerError<GetDocUpdatesError>> {
        let request = &context.request;
        let owner = Owner(context.public_key);
        let requested: HashSet<Uuid> = request.ids.iter().copied().collect();

        let mut lock = self.index_db.lock().await;
        let mut tx = ServerTx::begin(&mut lock)?;
        let db = &mut *tx;

        let doc_info = &db.doc_info;
        let mut tree = ServerTree::new(
            owner,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();

        let mut heads = vec![];
        for id in tree.ids() {
            let Some(info) = doc_info.get(&id) else {
                continue;
            };
            if info.updated_at < request.since && !requested.contains(&id) {
                continue;
            }
            if tree.calculate_deleted(&id)? {
                continue;
            }
            if let Some(head) = info.summary() {
                heads.push((id, head));
            }
        }

        Ok(GetDocUpdatesResponse { as_of: get_time().0 as u64, heads })
    }
}

pub fn retained(info: Option<&DocInfo>, key: &BlobKey) -> bool {
    info.is_some_and(|info| info.references(key))
}
