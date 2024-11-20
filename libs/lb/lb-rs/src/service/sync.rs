use super::network::ApiError;
use crate::logic::file_like::FileLike;
use crate::logic::filename::{DocumentType, NameComponents};
use crate::logic::signed_file::SignedFile;
use crate::logic::staged::StagedTreeLikeMut;
use crate::logic::tree_like::TreeLike;
use crate::logic::{symkey, SharedErrorKind, ValidationFailure};
use crate::model::access_info::UserAccessMode;
use crate::model::api::{
    ChangeDocRequest, GetDocRequest, GetFileIdsRequest, GetUpdatesRequest, GetUpdatesResponse,
    GetUsernameError, GetUsernameRequest, UpsertRequest,
};
use crate::model::clock;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::ShareMode;
use crate::model::file_metadata::{DocumentHmac, FileDiff, FileType, Owner};
use crate::model::work_unit::WorkUnit;
use crate::svg::buffer::u_transform_to_bezier;
use crate::svg::element::Element;
use crate::text::buffer::Buffer;
use crate::Lb;
pub use basic_human_duration::ChronoHumanDuration;
use futures::stream;
use futures::StreamExt;
use serde::Serialize;
use std::collections::{hash_map, HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use time::Duration;
use uuid::Uuid;

pub type SyncFlag = Arc<AtomicBool>;

pub struct SyncContext {
    progress: Option<Box<dyn Fn(SyncProgress) + Send>>,
    current: usize,
    total: usize,

    pk_cache: HashMap<Owner, String>,
    last_synced: u64,
    remote_changes: Vec<SignedFile>,
    update_as_of: u64,
    root: Option<Uuid>,
    pushed_metas: Vec<FileDiff<SignedFile>>,
    pushed_docs: Vec<FileDiff<SignedFile>>,
}

impl Lb {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub async fn calculate_work(&self) -> LbResult<SyncStatus> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        let last_synced = db.last_synced.get().copied().unwrap_or_default() as u64;
        drop(tx);

        let remote_changes = self
            .client
            .request(self.get_account()?, GetUpdatesRequest { since_metadata_version: last_synced })
            .await?;
        let (deduped, latest_server_ts, _) = self.dedup(remote_changes).await?;
        let remote_dirty = deduped
            .into_iter()
            .map(|f| *f.id())
            .map(WorkUnit::ServerChange);

        self.prune().await?;

        let tx = self.ro_tx().await;
        let db = tx.db();

        let locally_dirty = db
            .local_metadata
            .get()
            .keys()
            .copied()
            .map(WorkUnit::LocalChange);

        let mut work_units: Vec<WorkUnit> = Vec::new();
        work_units.extend(locally_dirty.chain(remote_dirty));
        Ok(SyncStatus { work_units, latest_server_ts })
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub async fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus> {
        let mut ctx = self.setup_sync(f).await?;

        let mut got_updates = false;
        let mut pipeline: LbResult<()> = async {
            ctx.msg("Preparing Sync...");
            self.prune().await?;
            got_updates = self.fetch_meta(&mut ctx).await?;
            self.populate_pk_cache(&mut ctx).await?;
            self.docs.dont_delete.store(true, Ordering::SeqCst);
            self.fetch_docs(&mut ctx).await?;
            self.merge(&mut ctx).await?;
            self.push_meta(&mut ctx).await?;
            self.push_docs(&mut ctx).await?;
            Ok(())
        }
        .await;

        self.docs.dont_delete.store(false, Ordering::SeqCst);

        if pipeline.is_ok() {
            pipeline = self.commit_last_synced(&mut ctx).await;
        }

        let cleanup = self.cleanup().await;

        pipeline?;
        cleanup?;
        ctx.done_msg();

        if got_updates {
            self.spawn_build_index();
        }

        Ok(ctx.summarize())
    }

    async fn setup_sync(
        &self, progress: Option<Box<dyn Fn(SyncProgress) + Send>>,
    ) -> LbResult<SyncContext> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let last_synced = db.last_synced.get().copied().unwrap_or_default() as u64;
        let pk_cache = db.pub_key_lookup.get().clone();

        let current = 0;
        let total = 7;

        Ok(SyncContext {
            last_synced,
            pk_cache,

            progress,
            current,
            total,

            root: Default::default(),
            update_as_of: Default::default(),
            remote_changes: Default::default(),
            pushed_docs: Default::default(),
            pushed_metas: Default::default(),
        })
    }

    async fn prune(&self) -> LbResult<()> {
        let server_ids = self
            .client
            .request(self.get_account()?, GetFileIdsRequest {})
            .await?
            .ids;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut local = db.base_metadata.stage(&db.local_metadata).to_lazy();
        let base_ids = local.tree.base.owned_ids();

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?.into_iter());
        }
        for id in &prunable_ids {
            if let Some(base_file) = local.tree.base.maybe_find(id) {
                self.docs
                    .delete(*id, base_file.document_hmac().copied())
                    .await?;
            }
            if let Some(local_file) = local.maybe_find(id) {
                self.docs
                    .delete(*id, local_file.document_hmac().copied())
                    .await?;
            }
        }

        let mut base_staged = (&mut db.base_metadata).to_lazy().stage(None);
        base_staged.tree.removed = prunable_ids.clone();
        base_staged.promote()?;

        let mut local_staged = (&mut db.local_metadata).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids;
        local_staged.promote()?;

        Ok(())
    }

    /// Returns true if there were any updates
    async fn fetch_meta(&self, ctx: &mut SyncContext) -> LbResult<bool> {
        ctx.msg("Fetching tree updates...");
        let updates = self
            .client
            .request(
                self.get_account()?,
                GetUpdatesRequest { since_metadata_version: ctx.last_synced },
            )
            .await?;

        let empty = updates.file_metadata.is_empty();
        let (remote, as_of, root) = self.dedup(updates).await?;
        ctx.remote_changes = remote;
        ctx.update_as_of = as_of;
        ctx.root = root;

        Ok(!empty)
    }

    async fn populate_pk_cache(&self, ctx: &mut SyncContext) -> LbResult<()> {
        ctx.msg("Updating public key cache...");
        let mut all_owners = HashSet::new();
        for file in &ctx.remote_changes {
            for user_access_key in file.user_access_keys() {
                all_owners.insert(Owner(user_access_key.encrypted_by));
                all_owners.insert(Owner(user_access_key.encrypted_for));
            }
        }

        let mut new_entries = HashMap::new();

        for owner in all_owners {
            if let hash_map::Entry::Vacant(e) = ctx.pk_cache.entry(owner) {
                let username_result = self
                    .client
                    .request(self.get_account()?, GetUsernameRequest { key: owner.0 })
                    .await;
                let username = match username_result {
                    Err(ApiError::Endpoint(GetUsernameError::UserNotFound)) => {
                        "<unknown>".to_string()
                    }
                    _ => username_result?.username.clone(),
                };
                new_entries.insert(owner, username.clone());
                e.insert(username.clone());
            }
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        for (owner, username) in new_entries {
            db.pub_key_lookup.insert(owner, username)?;
        }
        Ok(())
    }

    async fn fetch_docs(&self, ctx: &mut SyncContext) -> LbResult<()> {
        ctx.msg("Fetching documents...");
        let mut docs_to_pull = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut remote = db.base_metadata.stage(ctx.remote_changes.clone()).to_lazy(); // this used to be owned remote changes
        for id in remote.tree.staged.owned_ids() {
            if remote.calculate_deleted(&id)? {
                continue;
            }
            let remote_hmac = remote.find(&id)?.document_hmac().cloned();
            let base_hmac = remote
                .tree
                .base
                .maybe_find(&id)
                .and_then(|f| f.document_hmac())
                .cloned();
            if base_hmac == remote_hmac {
                continue;
            }

            if let Some(remote_hmac) = remote_hmac {
                docs_to_pull.push((id, remote_hmac));
            }
        }
        drop(tx);

        let num_docs = docs_to_pull.len();
        ctx.total += num_docs;

        let futures = docs_to_pull
            .into_iter()
            .map(|(id, hmac)| self.fetch_doc(id, hmac));

        let mut stream = stream::iter(futures).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        let mut idx = 0;
        while let Some(fut) = stream.next().await {
            let id = fut?;
            ctx.file_msg(id, &format!("Downloaded file {idx} of {num_docs}."));
            idx += 1;
        }
        Ok(())
    }

    async fn fetch_doc(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<Uuid> {
        let remote_document = self
            .client
            .request(self.get_account()?, GetDocRequest { id, hmac })
            .await?;
        self.docs
            .insert(id, Some(hmac), &remote_document.content)
            .await?;

        Ok(id)
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    async fn merge(&self, ctx: &mut SyncContext) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let remote_changes = &ctx.remote_changes;

        // fetch document updates and local documents for merge
        let me = Owner(self.get_pk()?);

        // compute merge changes
        let merge_changes = {
            // assemble trees
            let mut base = (&db.base_metadata).to_lazy();
            let remote_unlazy = (&db.base_metadata).to_staged(remote_changes);
            let mut remote = remote_unlazy.as_lazy();
            let mut local = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

            // changeset constraints - these evolve as we try to assemble changes and encounter validation failures
            let mut files_to_unmove: HashSet<Uuid> = HashSet::new();
            let mut files_to_unshare: HashSet<Uuid> = HashSet::new();
            let mut links_to_delete: HashSet<Uuid> = HashSet::new();
            let mut rename_increments: HashMap<Uuid, usize> = HashMap::new();
            let mut duplicate_file_ids: HashMap<Uuid, Uuid> = HashMap::new();

            'merge_construction: loop {
                // process just the edits which allow us to check deletions in the result
                let mut deletions = {
                    let mut deletions = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations
                    let mut deletion_creations = HashSet::new();
                    for id in db.local_metadata.owned_ids() {
                        if remote.maybe_find(&id).is_none() && !links_to_delete.contains(&id) {
                            deletion_creations.insert(id);
                        }
                    }
                    'drain_creations: while !deletion_creations.is_empty() {
                        'choose_a_creation: for id in &deletion_creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            let result = deletions.create_unvalidated(
                                id,
                                symkey::generate_key(),
                                local_file.parent(),
                                &local.name(&id, self.get_account()?)?,
                                local_file.file_type(),
                                self.get_account()?,
                            );
                            match result {
                                Ok(_) => {
                                    deletion_creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    SharedErrorKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(LbErrKind::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            deletion_creations
                        ))
                        .into());
                    }

                    // moves (creations happen first in case a file is moved into a new folder)
                    for id in db.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = db.base_metadata.maybe_find(&id).cloned() {
                            if !local_file.explicitly_deleted()
                                && local_file.parent() != base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                // move
                                deletions.move_unvalidated(
                                    &id,
                                    local_file.parent(),
                                    self.get_account()?,
                                )?;
                            }
                        }
                    }

                    // deletions (moves happen first in case a file is moved into a deleted folder)
                    for id in db.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if local_file.explicitly_deleted() {
                            // delete
                            deletions.delete_unvalidated(&id, self.get_account()?)?;
                        }
                    }
                    deletions
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                let mut merge = {
                    let mut merge = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations and edits of created documents
                    let mut creations = HashSet::new();
                    for id in db.local_metadata.owned_ids() {
                        if deletions.maybe_find(&id).is_some()
                            && !deletions.calculate_deleted(&id)?
                            && remote.maybe_find(&id).is_none()
                            && !links_to_delete.contains(&id)
                        {
                            creations.insert(id);
                        }
                    }
                    'drain_creations: while !creations.is_empty() {
                        'choose_a_creation: for id in &creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            let result = merge.create_unvalidated(
                                id,
                                local.decrypt_key(&id, self.get_account()?)?,
                                local_file.parent(),
                                &local.name(&id, self.get_account()?)?,
                                local_file.file_type(),
                                self.get_account()?,
                            );
                            match result {
                                Ok(_) => {
                                    creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    SharedErrorKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(LbErrKind::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            creations
                        ))
                        .into());
                    }

                    // moves, renames, edits, and shares
                    // creations happen first in case a file is moved into a new folder
                    for id in db.local_metadata.owned_ids() {
                        // skip files that are already deleted or will be deleted
                        if deletions.maybe_find(&id).is_none()
                            || deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                        {
                            continue;
                        }

                        let local_file = local.find(&id)?.clone();
                        let local_name = local.name(&id, self.get_account()?)?;
                        let maybe_base_file = base.maybe_find(&id).cloned();
                        let maybe_remote_file = remote.maybe_find(&id).cloned();
                        if let Some(ref base_file) = maybe_base_file {
                            let base_name = base.name(&id, self.get_account()?)?;
                            let remote_file = remote.find(&id)?.clone();
                            let remote_name = remote.name(&id, self.get_account()?)?;

                            // move
                            if local_file.parent() != base_file.parent()
                                && remote_file.parent() == base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                merge.move_unvalidated(
                                    &id,
                                    local_file.parent(),
                                    self.get_account()?,
                                )?;
                            }

                            // rename
                            if local_name != base_name && remote_name == base_name {
                                merge.rename_unvalidated(&id, &local_name, self.get_account()?)?;
                            }
                        }

                        // share
                        let mut remote_keys = HashMap::new();
                        if let Some(ref remote_file) = maybe_remote_file {
                            for key in remote_file.user_access_keys() {
                                remote_keys.insert(
                                    (Owner(key.encrypted_by), Owner(key.encrypted_for)),
                                    (key.mode, key.deleted),
                                );
                            }
                        }
                        for key in local_file.user_access_keys() {
                            let (by, for_) = (Owner(key.encrypted_by), Owner(key.encrypted_for));
                            if let Some(&(remote_mode, remote_deleted)) =
                                remote_keys.get(&(by, for_))
                            {
                                // upgrade share
                                if key.mode > remote_mode || !key.deleted && remote_deleted {
                                    let mode = match key.mode {
                                        UserAccessMode::Read => ShareMode::Read,
                                        UserAccessMode::Write => ShareMode::Write,
                                        UserAccessMode::Owner => continue,
                                    };
                                    merge.add_share_unvalidated(
                                        id,
                                        for_,
                                        mode,
                                        self.get_account()?,
                                    )?;
                                }
                                // delete share
                                if key.deleted && !remote_deleted {
                                    merge.delete_share_unvalidated(
                                        &id,
                                        Some(for_.0),
                                        self.get_account()?,
                                    )?;
                                }
                            } else {
                                // add share
                                let mode = match key.mode {
                                    UserAccessMode::Read => ShareMode::Read,
                                    UserAccessMode::Write => ShareMode::Write,
                                    UserAccessMode::Owner => continue,
                                };
                                merge.add_share_unvalidated(id, for_, mode, self.get_account()?)?;
                            }
                        }

                        // share deletion due to conflicts
                        if files_to_unshare.contains(&id) {
                            merge.delete_share_unvalidated(&id, None, self.get_account()?)?;
                        }

                        // rename due to path conflict
                        if let Some(&rename_increment) = rename_increments.get(&id) {
                            let name = NameComponents::from(&local_name)
                                .generate_incremented(rename_increment)
                                .to_name();
                            merge.rename_unvalidated(&id, &name, self.get_account()?)?;
                        }

                        // edit
                        let base_hmac = maybe_base_file.and_then(|f| f.document_hmac().cloned());
                        let remote_hmac =
                            maybe_remote_file.and_then(|f| f.document_hmac().cloned());
                        let local_hmac = local_file.document_hmac().cloned();
                        if merge.access_mode(me, &id)? >= Some(UserAccessMode::Write)
                            && local_hmac != base_hmac
                        {
                            if remote_hmac != base_hmac && remote_hmac != local_hmac {
                                // merge
                                let merge_name = merge.name(&id, self.get_account()?)?;
                                let document_type =
                                    DocumentType::from_file_name_using_extension(&merge_name);

                                // todo these accesses are potentially problematic
                                // maybe not if service/docs is the persion doing network io
                                let base_document =
                                    self.read_document_helper(id, &mut base).await?;
                                let remote_document =
                                    self.read_document_helper(id, &mut remote).await?;
                                let local_document =
                                    self.read_document_helper(id, &mut local).await?;

                                match document_type {
                                    DocumentType::Text => {
                                        // 3-way merge
                                        // todo: a couple more clones than necessary
                                        let base_document =
                                            String::from_utf8_lossy(&base_document).to_string();
                                        let remote_document =
                                            String::from_utf8_lossy(&remote_document).to_string();
                                        let local_document =
                                            String::from_utf8_lossy(&local_document).to_string();
                                        let merged_document = Buffer::from(base_document.as_str())
                                            .merge(local_document, remote_document);
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document.into_bytes(),
                                                self.get_account()?,
                                            )?;
                                        let hmac = merge.find(&id)?.document_hmac().copied();
                                        self.docs.insert(id, hmac, &encrypted_document).await?;
                                    }
                                    DocumentType::Drawing => {
                                        println!("sync merge");
                                        let base_document =
                                            String::from_utf8_lossy(&base_document).to_string();
                                        let remote_document =
                                            String::from_utf8_lossy(&remote_document).to_string();

                                        let mut local_buffer = crate::svg::buffer::Buffer::new(
                                            String::from_utf8_lossy(&local_document).as_ref(),
                                            None,
                                            None,
                                        );

                                        for (_, el) in local_buffer.elements.iter_mut() {
                                            if let Element::Path(path) = el {
                                                path.data.apply_transform(u_transform_to_bezier(
                                                    &local_buffer.master_transform,
                                                ));
                                            }
                                        }
                                        crate::svg::buffer::Buffer::reload(
                                            &mut local_buffer.elements,
                                            local_buffer.master_transform,
                                            &base_document,
                                            &remote_document,
                                        );

                                        let merged_document = local_buffer.serialize();
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document.into_bytes(),
                                                self.get_account()?,
                                            )?;
                                        let hmac = merge.find(&id)?.document_hmac().copied();
                                        self.docs.insert(id, hmac, &encrypted_document).await?;
                                    }
                                    DocumentType::Other => {
                                        // duplicate file
                                        let merge_parent = *merge.find(&id)?.parent();
                                        let duplicate_id = if let Some(&duplicate_id) =
                                            duplicate_file_ids.get(&id)
                                        {
                                            duplicate_id
                                        } else {
                                            let duplicate_id = Uuid::new_v4();
                                            duplicate_file_ids.insert(id, duplicate_id);
                                            rename_increments.insert(duplicate_id, 1);
                                            duplicate_id
                                        };

                                        let mut merge_name = merge_name;
                                        merge_name = NameComponents::from(&merge_name)
                                            .generate_incremented(
                                                rename_increments
                                                    .get(&duplicate_id)
                                                    .copied()
                                                    .unwrap_or_default(),
                                            )
                                            .to_name();

                                        merge.create_unvalidated(
                                            duplicate_id,
                                            symkey::generate_key(),
                                            &merge_parent,
                                            &merge_name,
                                            FileType::Document,
                                            self.get_account()?,
                                        )?;
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &duplicate_id,
                                                &local_document,
                                                self.get_account()?,
                                            )?;
                                        let duplicate_hmac =
                                            merge.find(&duplicate_id)?.document_hmac().copied();
                                        self.docs
                                            .insert(
                                                duplicate_id,
                                                duplicate_hmac,
                                                &encrypted_document,
                                            )
                                            .await?;
                                    }
                                }
                            } else {
                                // overwrite (todo: avoid reading/decrypting/encrypting document)
                                let document = self.read_document_helper(id, &mut local).await?;
                                merge.update_document_unvalidated(
                                    &id,
                                    &document,
                                    self.get_account()?,
                                )?;
                            }
                        }
                    }

                    // deletes
                    // moves happen first in case a file is moved into a deleted folder
                    for id in db.local_metadata.owned_ids() {
                        if db.base_metadata.maybe_find(&id).is_some()
                            && deletions.calculate_deleted(&id)?
                            && !merge.calculate_deleted(&id)?
                        {
                            // delete
                            merge.delete_unvalidated(&id, self.get_account()?)?;
                        }
                    }
                    for &id in &links_to_delete {
                        // delete
                        if merge.maybe_find(&id).is_some() && !merge.calculate_deleted(&id)? {
                            merge.delete_unvalidated(&id, self.get_account()?)?;
                        }
                    }

                    merge
                };

                // validate; handle failures by introducing changeset constraints
                for link in merge.owned_ids() {
                    if !merge.calculate_deleted(&link)? {
                        if let FileType::Link { target } = merge.find(&link)?.file_type() {
                            if merge.maybe_find(&target).is_some()
                                && merge.calculate_deleted(&target)?
                            {
                                // delete links to deleted files
                                if links_to_delete.insert(link) {
                                    continue 'merge_construction;
                                } else {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve broken link (deletion): {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                        }
                    }
                }

                let validate_result = merge.validate(me);
                match validate_result {
                    // merge changeset is valid
                    Ok(_) => {
                        let (_, merge_changes) = merge.unstage();
                        break merge_changes;
                    }
                    Err(ref err) => match err.kind {
                        SharedErrorKind::ValidationFailure(ref vf) => match vf {
                            // merge changeset has resolvable validation errors and needs modification
                            ValidationFailure::Cycle(ids) => {
                                // revert all local moves in the cycle
                                let mut progress = false;
                                for &id in ids {
                                    if db.local_metadata.maybe_find(&id).is_some()
                                        && files_to_unmove.insert(id)
                                    {
                                        progress = true;
                                    }
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve cycle: {:?}",
                                        ids
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::PathConflict(ids) => {
                                // pick one local id and generate a non-conflicting filename
                                let mut progress = false;
                                for &id in ids {
                                    if duplicate_file_ids.values().any(|&dup| dup == id) {
                                        *rename_increments.entry(id).or_insert(0) += 1;
                                        progress = true;
                                        break;
                                    }
                                }
                                if !progress {
                                    for &id in ids {
                                        if db.local_metadata.maybe_find(&id).is_some() {
                                            *rename_increments.entry(id).or_insert(0) += 1;
                                            progress = true;
                                            break;
                                        }
                                    }
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve path conflict: {:?}",
                                        ids
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::SharedLink { link, shared_ancestor } => {
                                // if ancestor is newly shared, delete share, otherwise delete link
                                let mut progress = false;
                                if let Some(base_shared_ancestor) = base.maybe_find(shared_ancestor)
                                {
                                    if !base_shared_ancestor.is_shared()
                                        && files_to_unshare.insert(*shared_ancestor)
                                    {
                                        progress = true;
                                    }
                                }
                                if !progress && links_to_delete.insert(*link) {
                                    progress = true;
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                    "sync failed to resolve shared link: link: {:?}, shared_ancestor: {:?}",
                                    link, shared_ancestor
                                )).into());
                                }
                            }
                            ValidationFailure::DuplicateLink { target } => {
                                // delete local link with this target
                                let mut progress = false;
                                if let Some(link) = local.linked_by(target)? {
                                    if links_to_delete.insert(link) {
                                        progress = true;
                                    }
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve duplicate link: target: {:?}",
                                        target
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::BrokenLink(link) => {
                                // delete local link with this target
                                if !links_to_delete.insert(*link) {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve broken link: {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::OwnedLink(link) => {
                                // if target is newly owned, unmove target, otherwise delete link
                                let mut progress = false;
                                if let Some(remote_link) = remote.maybe_find(link) {
                                    if let FileType::Link { target } = remote_link.file_type() {
                                        let remote_target = remote.find(&target)?;
                                        if remote_target.owner() != me
                                            && files_to_unmove.insert(target)
                                        {
                                            progress = true;
                                        }
                                    }
                                }
                                if !progress && links_to_delete.insert(*link) {
                                    progress = true;
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve owned link: {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                            // merge changeset has unexpected validation errors
                            ValidationFailure::Orphan(_)
                            | ValidationFailure::NonFolderWithChildren(_)
                            | ValidationFailure::FileWithDifferentOwnerParent(_)
                            | ValidationFailure::FileNameTooLong(_)
                            | ValidationFailure::NonDecryptableFileName(_) => {
                                validate_result?;
                            }
                        },
                        // merge changeset has unexpected errors
                        _ => {
                            validate_result?;
                        }
                    },
                }
            }
        };

        // base = remote; local = merge
        (&mut db.base_metadata)
            .to_staged(remote_changes.clone())
            .to_lazy()
            .promote()?;
        db.local_metadata.clear()?;
        (&mut db.local_metadata)
            .to_staged(merge_changes)
            .to_lazy()
            .promote()?;

        // todo who else calls this did they manage locks right?
        // self.cleanup_local_metadata()?;
        db.base_metadata.stage(&mut db.local_metadata).prune()?;
        Ok(())
    }

    /// Updates remote and base metadata to local.
    async fn push_meta(&self, ctx: &mut SyncContext) -> LbResult<()> {
        ctx.msg("Pushing tree changes...");
        let mut updates = vec![];
        let mut local_changes_no_digests = Vec::new();

        let tx = self.ro_tx().await;
        let db = tx.db();

        // remote = local
        let local = db.base_metadata.stage(&db.local_metadata).to_lazy();

        for id in local.tree.staged.owned_ids() {
            let mut local_change = local.tree.staged.find(&id)?.timestamped_value.value.clone();
            let maybe_base_file = local.tree.base.maybe_find(&id);

            // change everything but document hmac and re-sign
            local_change.document_hmac =
                maybe_base_file.and_then(|f| f.timestamped_value.value.document_hmac);
            let local_change = local_change.sign(self.get_account()?)?;

            local_changes_no_digests.push(local_change.clone());
            let file_diff = FileDiff { old: maybe_base_file.cloned(), new: local_change };
            updates.push(file_diff);
        }

        drop(tx);

        if !updates.is_empty() {
            self.client
                .request(self.get_account()?, UpsertRequest { updates: updates.clone() })
                .await?;
            ctx.pushed_metas = updates;
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        // base = local
        (&mut db.base_metadata)
            .to_lazy()
            .stage(local_changes_no_digests)
            .promote()?;
        db.base_metadata.stage(&mut db.local_metadata).prune()?;

        tx.end();

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    async fn push_docs(&self, ctx: &mut SyncContext) -> LbResult<()> {
        ctx.msg("Pushing document changes...");
        let mut updates = vec![];
        let mut local_changes_digests_only = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();

        let local = db.base_metadata.stage(&db.local_metadata).to_lazy();

        for id in local.tree.staged.owned_ids() {
            let base_file = local.tree.base.find(&id)?.clone();

            // change only document hmac and re-sign
            let mut local_change = base_file.timestamped_value.value.clone();
            local_change.document_hmac = local.find(&id)?.timestamped_value.value.document_hmac;

            if base_file.document_hmac() == local_change.document_hmac()
                || local_change.document_hmac.is_none()
            {
                continue;
            }

            let local_change = local_change.sign(self.get_account()?)?;

            updates.push(FileDiff { old: Some(base_file), new: local_change.clone() });
            local_changes_digests_only.push(local_change);
        }

        drop(tx);

        let docs_count = updates.len();
        ctx.total += docs_count;
        let futures = updates.clone().into_iter().map(|diff| self.push_doc(diff));

        let mut stream = stream::iter(futures).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        let mut idx = 0;
        while let Some(fut) = stream.next().await {
            let id = fut?;
            ctx.file_msg(id, &format!("Pushed file {idx} of {docs_count}."));
            idx += 1;
        }
        ctx.pushed_docs = updates;

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        // base = local (metadata)
        (&mut db.base_metadata)
            .to_lazy()
            .stage(local_changes_digests_only)
            .promote()?;

        db.base_metadata.stage(&mut db.local_metadata).prune()?;

        tx.end();

        Ok(())
    }

    async fn push_doc(&self, diff: FileDiff<SignedFile>) -> LbResult<Uuid> {
        let id = *diff.new.id();
        let hmac = diff.new.document_hmac();
        let local_document_change = self.docs.get(id, hmac.copied()).await?;
        self.client
            .request(
                self.get_account()?,
                ChangeDocRequest { diff, new_content: local_document_change },
            )
            .await?;

        Ok(id)
    }

    async fn dedup(
        &self, updates: GetUpdatesResponse,
    ) -> LbResult<(Vec<SignedFile>, u64, Option<Uuid>)> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut root_id = None;
        let (remote_changes, update_as_of) = {
            let mut remote_changes = updates.file_metadata;
            let update_as_of = updates.as_of_metadata_version;

            remote_changes = self.prune_remote_orphans(remote_changes).await?;

            let remote = db.base_metadata.stage(remote_changes).pruned()?.to_lazy();

            let (_, remote_changes) = remote.unstage();
            (remote_changes, update_as_of)
        };

        // initialize root if this is the first pull on this device
        if db.root.get().is_none() {
            let root = remote_changes
                .all_files()?
                .into_iter()
                .find(|f| f.is_root())
                .ok_or(LbErrKind::RootNonexistent)?;
            root_id = Some(*root.id());
        }

        Ok((remote_changes, update_as_of, root_id))
    }

    async fn prune_remote_orphans(
        &self, remote_changes: Vec<SignedFile>,
    ) -> LbResult<Vec<SignedFile>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let me = Owner(self.get_pk()?);
        let remote = db.base_metadata.stage(remote_changes).to_lazy();
        let mut result = Vec::new();

        for id in remote.tree.staged.owned_ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || meta
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == me.0)
            {
                result.push(remote.find(&id)?.clone()); // todo: don't clone
            }
        }
        Ok(result)
    }

    async fn commit_last_synced(&self, ctx: &mut SyncContext) -> LbResult<()> {
        ctx.msg("Cleaning up...");
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.last_synced.insert(ctx.update_as_of as i64)?;

        if let Some(root) = ctx.root {
            db.root.insert(root)?;
        }

        Ok(())
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        let last_synced = db.last_synced.get().copied().unwrap_or(0);

        Ok(self.get_timestamp_human_string(last_synced))
    }

    pub fn get_timestamp_human_string(&self, timestamp: i64) -> String {
        if timestamp != 0 {
            Duration::milliseconds(clock::get_time().0 - timestamp)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        }
    }
}

impl SyncContext {
    fn summarize(&self) -> SyncStatus {
        let mut local = HashSet::new();
        let mut server = HashSet::new();
        let mut work_units = vec![];

        for meta in &self.pushed_metas {
            local.insert(meta.new.id());
        }

        for meta in &self.pushed_docs {
            local.insert(meta.new.id());
        }

        for meta in &self.remote_changes {
            server.insert(meta.id());
        }

        for id in local {
            work_units.push(WorkUnit::LocalChange(*id));
        }

        for id in server {
            work_units.push(WorkUnit::ServerChange(*id));
        }

        SyncStatus { work_units, latest_server_ts: self.update_as_of }
    }

    fn msg(&mut self, msg: &str) {
        self.current += 1;
        if let Some(f) = &self.progress {
            f(SyncProgress {
                total: self.total,
                progress: self.current,
                file_being_processed: Default::default(),
                msg: msg.to_string(),
            })
        }
    }

    fn file_msg(&mut self, id: Uuid, msg: &str) {
        self.current += 1;
        if let Some(f) = &self.progress {
            f(SyncProgress {
                total: self.total,
                progress: self.current,
                file_being_processed: Some(id),
                msg: msg.to_string(),
            })
        }
    }

    fn done_msg(&mut self) {
        self.current = self.total;
        if let Some(f) = &self.progress {
            f(SyncProgress {
                total: self.total,
                progress: self.current,
                file_being_processed: None,
                msg: "Sync successful!".to_string(),
            })
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncStatus {
    pub work_units: Vec<WorkUnit>,
    pub latest_server_ts: u64,
}

#[derive(Clone)]
pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub file_being_processed: Option<Uuid>,
    pub msg: String,
}

impl Display for SyncProgress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} / {}]: {}", self.progress, self.total, self.msg)
    }
}
