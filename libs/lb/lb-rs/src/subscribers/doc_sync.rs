use std::num::NonZeroUsize;
use std::thread;

use futures::{StreamExt, stream};
use usvg::Transform;
use uuid::Uuid;

use crate::Lb;
use crate::io::docs::PendingDoc;
use crate::io::network::ApiError;
use crate::io::server::Server;
use crate::model::ValidationFailure;
use crate::model::access_info::UserAccessMode;
use crate::model::api::{
    GetDocErrorV3, GetDocRequestV3, GetDocUpdatesRequest, PayloadKind, PushDocError, PushDocRequest,
};
use crate::model::chat;
use crate::model::clock::get_time;
use crate::model::crypto::{AESKey, EncryptedDocument};
use crate::model::doc_diff::DocDiff;
use crate::model::doc_info::{DocHead, DocVersion, LocalDocInfo, LocalVersion, VersionKind};
use crate::model::errors::{LbErr, LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{DocumentHmac, FileType, Owner};
use crate::model::filename::{DocumentType, NameComponents};
use crate::model::svg::{self, buffer::u_transform_to_bezier, element::Element};
use crate::model::tree_like::TreeLike;
use crate::model::{symkey, text};
use crate::service::documents::{
    compress_encrypt_document, decrypt_payload, document_hmac, encrypt_diff_payload,
    encrypt_payload,
};
use crate::service::events::{Actor, SyncIncrement};

const ATTEMPTS: usize = 3;
const MAX_DUPLICATE_NAMES: usize = 1000;

enum Merge {
    Merged(Vec<u8>),
    Conflict,
}

enum Rebase {
    Clear,
    Local { hmac: DocumentHmac, compressed_size: u64, pending: PendingDoc },
    Duplicate { id: Uuid, key: AESKey, local: LocalVersion, pending: PendingDoc },
}

struct PullTarget {
    key: AESKey,
    info: LocalDocInfo,
    name: String,
    parent: Uuid,
}

impl<S: Server> Lb<S> {
    pub(crate) async fn pull_doc_updates(&self) -> LbResult<()> {
        let (since, unknown) = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            let since = db.last_doc_synced.as_ref().copied().unwrap_or_default();
            let unknown: Vec<Uuid> = db
                .base_metadata
                .iter()
                .filter(|(id, meta)| meta.is_document() && !db.doc_info.contains_key(*id))
                .map(|(id, _)| *id)
                .collect();
            (since, unknown)
        };

        let response = self
            .client
            .get_doc_updates(
                self.get_account()?,
                GetDocUpdatesRequest { since, ids: unknown.clone() },
            )
            .await?;

        let mut heads_changed = false;
        {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            for (id, head) in response.heads {
                let mut info = db.doc_info.get(&id).copied().unwrap_or_default();
                let remote = newer(info.remote, head);
                if remote != info.remote || !db.doc_info.contains_key(&id) {
                    info.remote = remote;
                    db.doc_info.insert(id, info)?;
                    heads_changed = true;
                }
            }
            for id in unknown {
                if !db.doc_info.contains_key(&id) {
                    db.doc_info.insert(id, LocalDocInfo::default())?;
                }
            }
            db.last_doc_synced.replace(response.as_of)?;
            tx.end();
        }
        if heads_changed {
            self.events.meta_changed(Actor::Sync);
        }

        let to_pull: Vec<Uuid> = {
            let tx = self.ro_tx().await;
            tx.db()
                .doc_info
                .iter()
                .filter(|(id, info)| {
                    info.is_behind()
                        && (info.local.is_some() || self.docs.exists(**id, info.base_hmac()))
                })
                .map(|(id, _)| *id)
                .collect()
        };
        self.for_each_doc(to_pull, |id| self.pull_doc(id)).await
    }

    pub(crate) async fn push_doc_updates(&self) -> LbResult<()> {
        let to_push: Vec<Uuid> = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            db.doc_info
                .iter()
                .filter(|(id, info)| info.local.is_some() && db.base_metadata.get(*id).is_some())
                .map(|(id, _)| *id)
                .collect()
        };
        self.for_each_doc(to_push, |id| self.push_doc(id)).await
    }

    pub(crate) async fn fetch_doc_contents(&self) -> LbResult<()> {
        let to_fetch: Vec<Uuid> = {
            let tx = self.ro_tx().await;
            let db = tx.db();

            let Some(root) = db.root.as_ref() else {
                return Ok(());
            };

            let mut tree = db.base_metadata.stage(None).to_lazy();
            let mut to_fetch = vec![];
            for id in tree.descendants_using_links(root)? {
                let file = tree.find(&id)?;
                if !file.is_document() || tree.calculate_deleted(&id)? {
                    continue;
                }
                let info = db.doc_info.get(&id).copied().unwrap_or_default();
                let missing = info.local.is_none()
                    && (info.is_behind()
                        || (info.base.is_some() && !self.docs.exists(id, info.base_hmac())));
                if !missing {
                    continue;
                }
                let name = tree.name(&id, &self.keychain)?;
                if name.ends_with(".md") || name.ends_with(".svg") {
                    to_fetch.push(id);
                }
            }
            to_fetch
        };

        for id in to_fetch {
            self.pull_doc(id).await?;
        }
        Ok(())
    }

    async fn for_each_doc<'a, F, Fut>(&'a self, ids: Vec<Uuid>, f: F) -> LbResult<()>
    where
        F: Fn(Uuid) -> Fut,
        Fut: std::future::Future<Output = LbResult<()>> + 'a,
    {
        let mut results = stream::iter(ids.into_iter().map(f)).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        let mut last_error: Option<LbErr> = None;
        while let Some(result) = results.next().await {
            if let Err(err) = result {
                last_error = Some(err);
            }
        }
        match last_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub(crate) async fn pull_doc(&self, id: Uuid) -> LbResult<()> {
        self.pull_doc_with(id, false).await
    }

    async fn pull_doc_with(&self, id: Uuid, refresh: bool) -> LbResult<()> {
        for _ in 0..ATTEMPTS {
            if self.try_pull_doc(id, refresh).await? {
                return Ok(());
            }
        }
        Err(LbErrKind::Unexpected(format!("document {id} kept changing while it was pulled"))
            .into())
    }

    async fn pull_target(&self, id: Uuid) -> LbResult<Option<PullTarget>> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let Some((is_document, parent)) =
            tree.maybe_find(&id).map(|f| (f.is_document(), *f.parent()))
        else {
            return Ok(None);
        };
        if !is_document || tree.calculate_deleted(&id)? {
            return Ok(None);
        }
        Ok(Some(PullTarget {
            key: tree.decrypt_key(&id, &self.keychain)?,
            info: db.doc_info.get(&id).copied().unwrap_or_default(),
            name: tree.name(&id, &self.keychain)?,
            parent,
        }))
    }

    async fn try_pull_doc(&self, id: Uuid, refresh: bool) -> LbResult<bool> {
        let Some(PullTarget { key, info, name, parent }) = self.pull_target(id).await? else {
            return Ok(true);
        };

        let have = info
            .base_hmac()
            .filter(|hmac| self.docs.exists(id, Some(*hmac)));
        if !refresh && have.is_some() && !info.is_behind() {
            return Ok(true);
        }
        let have_content = match have {
            Some(hmac) => Some(decrypt_payload(&key, &self.docs.get(id, Some(hmac)).await?)?),
            None => None,
        };

        self.events
            .sync_update(SyncIncrement::PullingDocument(id, true));
        let response = self
            .client
            .get_doc_v3(self.get_account()?, GetDocRequestV3 { id, have, want: None })
            .await;
        self.events
            .sync_update(SyncIncrement::PullingDocument(id, false));
        let response = match response {
            Ok(response) => response,
            Err(ApiError::Endpoint(
                GetDocErrorV3::DocumentNotFound | GetDocErrorV3::NotPermissioned,
            )) => return Ok(true),
            Err(err) => return Err(err.into()),
        };

        let Some(head) = response.head else {
            let mut tx = self.begin_tx().await;
            let db = tx.db();
            if !db.doc_info.contains_key(&id) {
                db.doc_info.insert(id, LocalDocInfo::default())?;
            }
            tx.end();
            return Ok(true);
        };

        let remote_content =
            materialize(&key, have_content.clone(), &response.versions, &head.hmac)?;
        let remote_pending = if have == Some(head.hmac) {
            None
        } else {
            let blob = match response.versions.last() {
                Some((version, payload)) if version.is_snapshot() && version.hmac == head.hmac => {
                    payload.clone()
                }
                _ => encrypt_payload(&key, &remote_content)?,
            };
            Some(self.docs.stage(id, head.hmac, &blob).await?)
        };

        let rebase = match info.local {
            None => Rebase::Clear,
            Some(local) if local.hmac == head.hmac => Rebase::Clear,
            Some(local) => {
                let local_content =
                    decrypt_payload(&key, &self.docs.get(id, Some(local.hmac)).await?)?;
                let base_content = match info.base_hmac() {
                    Some(base) if Some(base) == have => have_content,
                    Some(base) => self.version_content(id, &key, base).await?,
                    None => None,
                };
                match merge_contents(&name, base_content, &local_content, &remote_content) {
                    Merge::Merged(merged) => {
                        let (hmac, encrypted) = compress_encrypt_document(&key, &merged)?;
                        if hmac == head.hmac {
                            Rebase::Clear
                        } else {
                            Rebase::Local {
                                hmac,
                                compressed_size: encrypted.value.len() as u64,
                                pending: self.docs.stage(id, hmac, &encrypted).await?,
                            }
                        }
                    }
                    Merge::Conflict => {
                        let duplicate_id = Uuid::new_v4();
                        let duplicate_key = symkey::generate_key();
                        let (hmac, encrypted) =
                            compress_encrypt_document(&duplicate_key, &local_content)?;
                        Rebase::Duplicate {
                            id: duplicate_id,
                            key: duplicate_key,
                            local: LocalVersion {
                                hmac,
                                compressed_size: encrypted.value.len() as u64,
                                timestamp: get_time().0 as u64,
                            },
                            pending: self.docs.stage(duplicate_id, hmac, &encrypted).await?,
                        }
                    }
                }
            }
        };

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        let current = db.doc_info.get(&id).copied().unwrap_or_default();
        if current.base != info.base || current.local != info.local {
            drop(tx);
            if let Some(pending) = remote_pending {
                self.docs.discard(pending).await;
            }
            match rebase {
                Rebase::Local { pending, .. } | Rebase::Duplicate { pending, .. } => {
                    self.docs.discard(pending).await
                }
                Rebase::Clear => {}
            }
            return Ok(false);
        }

        if let Some(pending) = remote_pending {
            self.docs.commit(pending).await?;
        }
        let mut next = current;
        next.remote = newer(current.remote, head);
        next.base = Some(head);
        let mut created_duplicate = false;
        match rebase {
            Rebase::Clear => next.local = None,
            Rebase::Local { hmac, compressed_size, pending } => {
                self.docs.commit(pending).await?;
                next.local =
                    Some(LocalVersion { hmac, compressed_size, timestamp: get_time().0 as u64 });
            }
            Rebase::Duplicate { id: duplicate_id, key: duplicate_key, local, pending } => {
                let mut tree = (&db.base_metadata)
                    .to_staged(&mut db.local_metadata)
                    .to_lazy();
                let mut increment = 1;
                loop {
                    let duplicate_name = NameComponents::from(&name)
                        .generate_incremented(increment)
                        .to_name();
                    match tree.create(
                        duplicate_id,
                        duplicate_key,
                        &parent,
                        &duplicate_name,
                        FileType::Document,
                        &self.keychain,
                    ) {
                        Ok(_) => break,
                        Err(err)
                            if matches!(
                                err.kind,
                                LbErrKind::Validation(ValidationFailure::PathConflict(_))
                            ) && increment < MAX_DUPLICATE_NAMES =>
                        {
                            increment += 1;
                        }
                        Err(err) => return Err(err),
                    }
                }
                self.docs.commit(pending).await?;
                db.doc_info.insert(
                    duplicate_id,
                    LocalDocInfo { remote: None, base: None, local: Some(local) },
                )?;
                next.local = None;
                created_duplicate = true;
            }
        }
        let content_changed = next.current_hmac() != info.current_hmac();
        db.doc_info.insert(id, next)?;
        tx.end();

        if created_duplicate {
            self.events.meta_changed(Actor::Sync);
        }
        if content_changed {
            self.events.doc_written(id, Actor::Sync);
        }
        Ok(true)
    }

    async fn version_content(
        &self, id: Uuid, key: &AESKey, hmac: DocumentHmac,
    ) -> LbResult<Option<Vec<u8>>> {
        if let Some(blob) = self.docs.maybe_get(id, Some(hmac)).await? {
            return Ok(Some(decrypt_payload(key, &blob)?));
        }
        let response = self
            .client
            .get_doc_v3(self.get_account()?, GetDocRequestV3 { id, have: None, want: Some(hmac) })
            .await;
        match response {
            Ok(response) => Ok(Some(materialize(key, None, &response.versions, &hmac)?)),
            Err(ApiError::Endpoint(
                GetDocErrorV3::VersionNotFound
                | GetDocErrorV3::DocumentNotFound
                | GetDocErrorV3::NotPermissioned,
            )) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) async fn push_doc(&self, id: Uuid) -> LbResult<()> {
        let mut snapshot_required = false;
        for _ in 0..ATTEMPTS {
            let (key, info) = {
                let tx = self.ro_tx().await;
                let db = tx.db();
                let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
                let info = db.doc_info.get(&id).copied().unwrap_or_default();
                let Some(is_document) = tree.maybe_find(&id).map(|f| f.is_document()) else {
                    return Ok(());
                };
                let writable = is_document
                    && !tree.calculate_deleted(&id)?
                    && tree.access_mode(Owner(self.keychain.get_pk()?), &id)?
                        >= Some(UserAccessMode::Write);
                if !writable {
                    drop(tx);
                    return self.drop_local_version(id, info).await;
                }
                (tree.decrypt_key(&id, &self.keychain)?, info)
            };
            let Some(local) = info.local else {
                return Ok(());
            };

            let local_blob = self.docs.get(id, Some(local.hmac)).await?;
            let local_content = decrypt_payload(&key, &local_blob)?;
            let diff = if snapshot_required {
                None
            } else {
                self.diff_payload(id, &key, &info, &local_content, &local_blob)
                    .await?
            };
            let (kind, content, uncompressed_size) = match diff {
                Some((payload, uncompressed_size)) => {
                    (PayloadKind::Diff, payload, uncompressed_size)
                }
                None => (PayloadKind::Snapshot, local_blob, local_content.len() as u64),
            };

            self.events
                .sync_update(SyncIncrement::PushingDocument(id, true));
            let response = self
                .client
                .push_doc(
                    self.get_account()?,
                    PushDocRequest {
                        id,
                        base: info.base_hmac(),
                        hmac: local.hmac,
                        kind,
                        content,
                        uncompressed_size,
                        content_size: local_content.len() as u64,
                    },
                )
                .await;
            self.events
                .sync_update(SyncIncrement::PushingDocument(id, false));

            match response {
                Ok(response) => {
                    let mut tx = self.begin_tx().await;
                    let db = tx.db();
                    let mut current = db.doc_info.get(&id).copied().unwrap_or_default();
                    current.base = Some(response.head);
                    current.remote = newer(current.remote, response.head);
                    if current.local.map(|local| local.hmac) == Some(local.hmac) {
                        current.local = None;
                    }
                    db.doc_info.insert(id, current)?;
                    tx.end();
                    return Ok(());
                }
                Err(ApiError::Endpoint(PushDocError::SnapshotRequired)) => {
                    snapshot_required = true;
                }
                Err(ApiError::Endpoint(PushDocError::Conflict)) => {
                    self.pull_doc_with(id, true).await?;
                }
                Err(ApiError::Endpoint(
                    PushDocError::DocumentNotFound
                    | PushDocError::DocumentDeleted
                    | PushDocError::NotPermissioned
                    | PushDocError::NotADocument,
                )) => return Ok(()),
                Err(err) => return Err(err.into()),
            }
        }
        Err(LbErrKind::Unexpected(format!("document {id} kept conflicting while it was pushed"))
            .into())
    }

    async fn diff_payload(
        &self, id: Uuid, key: &AESKey, info: &LocalDocInfo, local_content: &[u8],
        local_blob: &EncryptedDocument,
    ) -> LbResult<Option<(EncryptedDocument, u64)>> {
        let Some(base) = info.base_hmac() else {
            return Ok(None);
        };
        let Some(base_blob) = self.docs.maybe_get(id, Some(base)).await? else {
            return Ok(None);
        };
        let base_content = decrypt_payload(key, &base_blob)?;
        let diff = DocDiff::compute(&base_content, local_content);
        if diff.apply(&base_content)? != local_content {
            return Ok(None);
        }
        let encoded = diff.encode()?;
        let payload = encrypt_diff_payload(key, &encoded, local_content)?;
        if payload.value.len() >= local_blob.value.len() {
            return Ok(None);
        }
        Ok(Some((payload, encoded.len() as u64)))
    }

    async fn drop_local_version(&self, id: Uuid, seen: LocalDocInfo) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        if let Some(mut current) = db.doc_info.get(&id).copied() {
            if current.local == seen.local && current.local.is_some() {
                current.local = None;
                db.doc_info.insert(id, current)?;
            }
        }
        tx.end();
        Ok(())
    }
}

fn newer(current: Option<DocHead>, candidate: DocHead) -> Option<DocHead> {
    match current {
        Some(current) if current.timestamp > candidate.timestamp => Some(current),
        _ => Some(candidate),
    }
}

fn materialize(
    key: &AESKey, mut content: Option<Vec<u8>>, versions: &[(DocVersion, EncryptedDocument)],
    target: &DocumentHmac,
) -> LbResult<Vec<u8>> {
    for (version, payload) in versions {
        let plain = decrypt_payload(key, payload)?;
        content = Some(match version.kind {
            VersionKind::Snapshot => plain,
            VersionKind::Diff { .. } => {
                let base = content.as_deref().ok_or_else(|| {
                    LbErrKind::Unexpected("received a diff without its base".to_string())
                })?;
                DocDiff::decode(&plain)?.apply(base)?
            }
        });
    }
    let content = content.unwrap_or_default();
    if &document_hmac(key, &content)? != target {
        return Err(LbErrKind::Unexpected(
            "pulled document content does not match its version".to_string(),
        )
        .into());
    }
    Ok(content)
}

fn merge_contents(name: &str, base: Option<Vec<u8>>, local: &[u8], remote: &[u8]) -> Merge {
    let Some(base) = base else {
        return Merge::Conflict;
    };
    match DocumentType::from_file_name_using_extension(name) {
        DocumentType::Text => {
            let base = String::from_utf8_lossy(&base).to_string();
            let local = String::from_utf8_lossy(local).to_string();
            let remote = String::from_utf8_lossy(remote).to_string();
            Merge::Merged(
                text::buffer::Buffer::from(base.as_str())
                    .merge(local, remote)
                    .into_bytes(),
            )
        }
        DocumentType::Drawing => {
            let base = String::from_utf8_lossy(&base).to_string();
            let remote = String::from_utf8_lossy(remote).to_string();
            let local = String::from_utf8_lossy(local).to_string();

            let base_buffer = svg::buffer::Buffer::new(&base);
            let remote_buffer = svg::buffer::Buffer::new(&remote);
            let mut local_buffer = svg::buffer::Buffer::new(&local);

            for (_, el) in local_buffer.elements.iter_mut() {
                if let Element::Path(path) = el {
                    path.data
                        .apply_transform(u_transform_to_bezier(&Transform::from(
                            local_buffer.weak_viewport_settings.master_transform,
                        )));
                }
            }
            svg::buffer::Buffer::reload(
                &mut local_buffer.elements,
                &mut local_buffer.weak_images,
                &mut local_buffer.weak_path_pressures,
                &mut local_buffer.weak_viewport_settings,
                &base_buffer,
                &remote_buffer,
            );

            Merge::Merged(local_buffer.serialize().into_bytes())
        }
        DocumentType::Chat => Merge::Merged(chat::Buffer::merge(&base, local, remote)),
        DocumentType::Other => Merge::Conflict,
    }
}
