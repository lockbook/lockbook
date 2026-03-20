use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use futures::{StreamExt, stream};
use tokio::sync::{Mutex, broadcast::error::TryRecvError};
use usvg::Transform;
use uuid::Uuid;

use crate::{
    Lb, LbErrKind, LbResult,
    io::network::ApiError,
    model::{
        ValidationFailure,
        access_info::UserAccessMode,
        account::Account,
        api::{
            ChangeDocRequestV2, GetDocRequest, GetFileIdsRequest, GetUpdatesRequestV2,
            GetUsernameError, GetUsernameRequest, UpsertDebugInfoRequest, UpsertRequestV2,
        },
        crypto::{DecryptedDocument, EncryptedDocument},
        errors::Unexpected,
        file::ShareMode,
        file_like::FileLike,
        file_metadata::{DocumentHmac, FileDiff, FileType, Owner},
        filename::{DocumentType, NameComponents},
        lazy::LazyTree,
        signed_meta::SignedMeta,
        staged::StagedTreeLikeMut,
        svg::{self, buffer::u_transform_to_bezier, element::Element},
        symkey, text,
        tree_like::TreeLike,
        validate,
    },
    service::events::{Actor, Event, SyncIncrement},
};

pub type Syncer = Arc<Mutex<SyncState>>;

#[derive(Default)]
pub struct SyncState {
    /// the starting point for updates for this sync pass
    last_synced: u64,

    /// if our pull is successful, this is the timestamp we will commit
    updates_as_of: u64,

    /// changes we pulled from the server, post deduplication
    remote_changes: Vec<SignedMeta>,

    /// did we pull a root on this pass?
    new_root: Option<Uuid>,

    /// what docs did we pull as a result of this sync
    pulled_docs: Vec<Uuid>,
}

// we are gonna have a fetch metadata fn which will get the docs that it needs to get, the ones
// that match should_fetch
//
// should_fetch is going to be a tree fn that will return true if:
//     is md or svg that descends from

impl Lb {
    pub async fn sync(&self) -> LbResult<()> {
        let mut sync_state = self.syncer.lock().await;

        let pipeline: LbResult<()> = async {
            self.pull_updates(&mut sync_state).await?;
            self.push_local_changes().await?;
            Ok(())
        }
        .await;

        self.events.sync_update(SyncIncrement::SyncFinished(
            pipeline.as_ref().err().map(|err| err.kind.clone()),
        ));

        self.cleanup().await?;

        pipeline?;

        let account = self.get_account()?.clone();
        if account.is_beta() {
            self.clone().send_debug_info(account);
        }

        Ok(())
    }

    pub(crate) async fn pull_updates(&self, sync_state: &mut SyncState) -> LbResult<()> {
        self.inital_sync_state(sync_state).await?;
        self.process_deletions().await?;
        self.fetch_meta(sync_state).await?;
        self.fetch_required_docs(sync_state).await?;
        // todo: should this inform a re-pull?
        self.merge(sync_state).await?;
        self.commit_last_synced(sync_state).await?;
        self.send_pull_events(sync_state).await?;

        if !self.config.background_work {
            self.populate_pk_cache().await?;
        }

        Ok(())
    }

    pub(crate) async fn push_local_changes(&self) -> LbResult<()> {
        self.push_meta().await?;
        self.push_docs().await?;

        Ok(())
    }

    async fn inital_sync_state(&self, state: &mut SyncState) -> LbResult<()> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        *state = Default::default();
        state.last_synced = db.last_synced.get().copied().unwrap_or_default() as u64;

        Ok(())
    }

    pub(crate) async fn process_deletions(&self) -> LbResult<()> {
        let server_ids = self
            .client
            .request(self.get_account()?, GetFileIdsRequest {})
            .await?
            .ids;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut local = db.base_metadata.stage(&db.local_metadata).to_lazy();
        let base_ids = local.tree.base.ids();

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
        base_staged.tree.removed = prunable_ids.iter().copied().collect();
        base_staged.promote()?;

        let mut local_staged = (&mut db.local_metadata).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids.iter().copied().collect();
        local_staged.promote()?;

        if !prunable_ids.is_empty() {
            self.events.meta_changed(Actor::Sync);
        }

        Ok(())
    }

    async fn fetch_meta(&self, state: &mut SyncState) -> LbResult<()> {
        let updates = self
            .client
            .request(
                self.get_account()?,
                GetUpdatesRequestV2 { since_metadata_version: state.last_synced },
            )
            .await?;

        let tx = self.ro_tx().await;
        let db = tx.db();

        // this loop implicitly prunes remote orphans
        let mut without_orphans = Vec::new();
        let me = Owner(self.keychain.get_pk()?);
        let remote = db.base_metadata.stage(updates.file_metadata).to_lazy();
        for id in remote.tree.staged.ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || meta
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == me.0)
            {
                without_orphans.push(remote.find(&id)?.clone());
            }
        }

        // this is what actually performs the deduplication
        let pruned_tree = db.base_metadata.stage(without_orphans).pruned()?.to_lazy();
        let (_, deduped_changes) = pruned_tree.unstage();

        // initialize root if this is the first pull on this device
        let mut root_id = None;
        if db.root.get().is_none() {
            let root = deduped_changes
                .all_files()?
                .into_iter()
                .find(|f| f.is_root())
                .ok_or(LbErrKind::RootNonexistent)?;
            root_id = Some(*root.id());
        }

        state.remote_changes = deduped_changes;
        state.updates_as_of = updates.as_of_metadata_version;
        state.new_root = root_id;

        Ok(())
    }

    async fn fetch_required_docs(&self, state: &mut SyncState) -> LbResult<()> {
        let mut docs_to_pull = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut files_with_local_edits = vec![];
        let local = db.base_metadata.stage(&db.local_metadata);
        for id in local.staged.ids() {
            if let Some(base) = local.base.maybe_find(&id) {
                if let Some(local_hmac) = local.find(&id)?.document_hmac() {
                    if Some(local_hmac) != base.document_hmac() {
                        files_with_local_edits.push(id);
                        println!("local edits found");
                    }
                }
            }
        }

        let mut remote = db
            .base_metadata
            .stage(state.remote_changes.clone())
            .to_lazy();

        for id in remote.tree.staged.ids() {
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
                // pull a file if we have a prior base, this is our heuristic -- do they have the
                // ability to edit this file while we release the lock and are pulling all the
                // files
                if self.docs.exists(id, base_hmac) && !self.docs.exists(id, Some(remote_hmac)) {
                    docs_to_pull.push((id, remote_hmac));
                }

                // this clause captures documents which went from being new -> multiple parties
                // having updates. We'll still need the updates
                if files_with_local_edits.contains(&id)
                    && !docs_to_pull
                        .iter()
                        .any(|(already_pulling, _)| already_pulling == &id)
                {
                    if let Some(base_hmac) = base_hmac {
                        if !self.docs.exists(id, Some(base_hmac)) {
                            // this scenario basically only comes up in tests
                            // someone modifies a file directly without reading the prior version
                            docs_to_pull.push((id, base_hmac));
                        }
                    }
                    docs_to_pull.push((id, remote_hmac));
                }
            }
        }
        drop(tx);

        let futures = docs_to_pull
            .into_iter()
            .map(|(id, hmac)| async move { self.fetch_doc(id, hmac).await.map(|_| id) });

        let mut stream = stream::iter(futures).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        while let Some(fut) = stream.next().await {
            let id = fut?;
            state.pulled_docs.push(id);
        }

        Ok(())
    }

    pub(crate) async fn fetch_doc(
        &self, id: Uuid, hmac: DocumentHmac,
    ) -> LbResult<EncryptedDocument> {
        // todo: in a lot of cases there is a list of ids we're trying to get, it would be better
        // if the caller managed the event updates, the status would be more meaningful for longer
        if let Ok(Some(doc)) = self.docs.maybe_get(id, Some(hmac)).await {
            return Ok(doc);
        }

        self.events
            .sync_update(SyncIncrement::PullingDocument(id, true));
        let remote_document = self
            .client
            .request(self.get_account()?, GetDocRequest { id, hmac })
            .await?;
        self.docs
            .insert(id, Some(hmac), &remote_document.content)
            .await?;
        self.events
            .sync_update(SyncIncrement::PullingDocument(id, false));

        Ok(remote_document.content)
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    async fn merge(&self, state: &mut SyncState) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        let start = Instant::now();

        let remote_changes = &state.remote_changes;

        // fetch document updates and local documents for merge
        let me = Owner(self.keychain.get_pk()?);

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
                    for id in db.local_metadata.ids() {
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
                                &local.name(&id, &self.keychain)?,
                                local_file.file_type(),
                                &self.keychain,
                            );
                            match result {
                                Ok(_) => {
                                    deletion_creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    LbErrKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(LbErrKind::Unexpected(format!(
                            "sync failed to find a topomodelal order for file creations: {deletion_creations:?}"
                        ))
                        .into());
                    }

                    // moves (creations happen first in case a file is moved into a new folder)
                    for id in db.local_metadata.ids() {
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
                                    &self.keychain,
                                )?;
                            }
                        }
                    }

                    // deletions (moves happen first in case a file is moved into a deleted folder)
                    for id in db.local_metadata.ids() {
                        let local_file = local.find(&id)?.clone();
                        if local_file.explicitly_deleted() {
                            // delete
                            deletions.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }
                    deletions
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                let mut merge = {
                    let mut merge = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations and edits of created documents
                    let mut creations = HashSet::new();
                    for id in db.local_metadata.ids() {
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
                                local.decrypt_key(&id, &self.keychain)?,
                                local_file.parent(),
                                &local.name(&id, &self.keychain)?,
                                local_file.file_type(),
                                &self.keychain,
                            );
                            match result {
                                Ok(_) => {
                                    creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    LbErrKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(LbErrKind::Unexpected(format!(
                            "sync failed to find a topomodelal order for file creations: {creations:?}"
                        ))
                        .into());
                    }

                    // moves, renames, edits, and shares
                    // creations happen first in case a file is moved into a new folder
                    for id in db.local_metadata.ids() {
                        // skip files that are already deleted or will be deleted
                        if deletions.maybe_find(&id).is_none()
                            || deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                        {
                            continue;
                        }

                        let local_file = local.find(&id)?.clone();
                        let local_name = local.name(&id, &self.keychain)?;
                        let maybe_base_file = base.maybe_find(&id).cloned();
                        let maybe_remote_file = remote.maybe_find(&id).cloned();
                        if let Some(ref base_file) = maybe_base_file {
                            let base_name = base.name(&id, &self.keychain)?;
                            let remote_file = remote.find(&id)?.clone();
                            let remote_name = remote.name(&id, &self.keychain)?;

                            // move
                            if local_file.parent() != base_file.parent()
                                && remote_file.parent() == base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                merge.move_unvalidated(&id, local_file.parent(), &self.keychain)?;
                            }

                            // rename
                            if local_name != base_name && remote_name == base_name {
                                merge.rename_unvalidated(&id, &local_name, &self.keychain)?;
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
                                    merge.add_share_unvalidated(id, for_, mode, &self.keychain)?;
                                }
                                // delete share
                                if key.deleted && !remote_deleted {
                                    merge.delete_share_unvalidated(
                                        &id,
                                        Some(for_.0),
                                        &self.keychain,
                                    )?;
                                }
                            } else {
                                // add share
                                let mode = match key.mode {
                                    UserAccessMode::Read => ShareMode::Read,
                                    UserAccessMode::Write => ShareMode::Write,
                                    UserAccessMode::Owner => continue,
                                };
                                merge.add_share_unvalidated(id, for_, mode, &self.keychain)?;
                            }
                        }

                        // share deletion due to conflicts
                        if files_to_unshare.contains(&id) {
                            merge.delete_share_unvalidated(&id, None, &self.keychain)?;
                        }

                        // rename due to path conflict
                        if let Some(&rename_increment) = rename_increments.get(&id) {
                            let name = NameComponents::from(&local_name)
                                .generate_incremented(rename_increment)
                                .to_name();
                            merge.rename_unvalidated(&id, &name, &self.keychain)?;
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
                                let merge_name = merge.name(&id, &self.keychain)?;
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
                                        let merged_document =
                                            text::buffer::Buffer::from(base_document.as_str())
                                                .merge(local_document, remote_document);
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document.into_bytes(),
                                                &self.keychain,
                                            )?;
                                        let hmac = merge.find(&id)?.document_hmac().copied();
                                        self.docs.insert(id, hmac, &encrypted_document).await?;
                                    }
                                    DocumentType::Drawing => {
                                        let base_document =
                                            String::from_utf8_lossy(&base_document).to_string();
                                        let remote_document =
                                            String::from_utf8_lossy(&remote_document).to_string();
                                        let local_document =
                                            String::from_utf8_lossy(&local_document).to_string();

                                        let base_buffer = svg::buffer::Buffer::new(&base_document);
                                        let remote_buffer =
                                            svg::buffer::Buffer::new(&remote_document);
                                        let mut local_buffer =
                                            svg::buffer::Buffer::new(&local_document);

                                        for (_, el) in local_buffer.elements.iter_mut() {
                                            if let Element::Path(path) = el {
                                                path.data.apply_transform(u_transform_to_bezier(
                                                    &Transform::from(
                                                        local_buffer
                                                            .weak_viewport_settings
                                                            .master_transform,
                                                    ),
                                                ));
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

                                        let merged_document = local_buffer.serialize();
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document.into_bytes(),
                                                &self.keychain,
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
                                            &self.keychain,
                                        )?;
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &duplicate_id,
                                                &local_document,
                                                &self.keychain,
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
                                    &self.keychain,
                                )?;
                            }
                        }
                    }

                    // deletes
                    // moves happen first in case a file is moved into a deleted folder
                    for id in db.local_metadata.ids() {
                        if db.base_metadata.maybe_find(&id).is_some()
                            && deletions.calculate_deleted(&id)?
                            && !merge.calculate_deleted(&id)?
                        {
                            // delete
                            merge.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }
                    for &id in &links_to_delete {
                        // delete
                        if merge.maybe_find(&id).is_some() && !merge.calculate_deleted(&id)? {
                            merge.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }

                    merge
                };

                // validate; handle failures by introducing changeset constraints
                for link in merge.ids() {
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
                                        "sync failed to resolve broken link (deletion): {link:?}"
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
                        LbErrKind::Validation(ref vf) => match vf {
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
                                        "sync failed to resolve cycle: {ids:?}"
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
                                        "sync failed to resolve path conflict: {ids:?}"
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
                                    "sync failed to resolve shared link: link: {link:?}, shared_ancestor: {shared_ancestor:?}"
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
                                        "sync failed to resolve duplicate link: target: {target:?}"
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::BrokenLink(link) => {
                                // delete local link with this target
                                if !links_to_delete.insert(*link) {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve broken link: {link:?}"
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
                                        "sync failed to resolve owned link: {link:?}"
                                    ))
                                    .into());
                                }
                            }
                            // merge changeset has unexpected validation errors
                            ValidationFailure::Orphan(_)
                            | ValidationFailure::NonFolderWithChildren(_)
                            | ValidationFailure::FileWithDifferentOwnerParent(_)
                            | ValidationFailure::FileNameTooLong(_)
                            | ValidationFailure::DeletedFileUpdated(_)
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

        if start.elapsed() > web_time::Duration::from_millis(100) {
            warn!("sync merge held lock for {:?}", start.elapsed());
        }

        Ok(())
    }

    async fn send_pull_events(&self, state: &mut SyncState) -> LbResult<()> {
        if !state.remote_changes.is_empty() {
            self.events.meta_changed(Actor::Sync);

            let owner = Owner(self.keychain.get_pk()?);
            if state.remote_changes.iter().any(|f| f.owner() != owner) {
                self.events.pending_shares_changed();
            }
        }

        for &doc in &state.pulled_docs {
            self.events.doc_written(doc, Actor::Sync);
        }

        Ok(())
    }

    async fn commit_last_synced(&self, state: &mut SyncState) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.last_synced.insert(state.updates_as_of as i64)?;

        if let Some(root) = state.new_root {
            db.root.insert(root)?;
        }

        Ok(())
    }

    async fn populate_pk_cache(&self) -> LbResult<()> {
        // todo: is this the move?
        let mut missing_owners = HashSet::new();
        {
            let tx = self.ro_tx().await;
            let db = tx.db();
            for file in db.base_metadata.get().values() {
                for user_access_key in file.user_access_keys() {
                    let enc_by = Owner(user_access_key.encrypted_by);
                    let enc_for = Owner(user_access_key.encrypted_for);

                    if !db.pub_key_lookup.get().contains_key(&enc_by) {
                        missing_owners.insert(enc_by);
                    }

                    if !db.pub_key_lookup.get().contains_key(&enc_for) {
                        missing_owners.insert(enc_for);
                    }
                }
            }
        }

        let mut new_owners = HashMap::new();
        {
            for owner in missing_owners {
                let username_result = self
                    .client
                    .request(self.get_account().unwrap(), GetUsernameRequest { key: owner.0 })
                    .await;
                new_owners.insert(owner, username_result);
            }
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let have_updates = !new_owners.is_empty();
        for (owner, username) in new_owners {
            let username = match username {
                Err(ApiError::Endpoint(GetUsernameError::UserNotFound)) => "<unknown>".to_string(),
                Ok(username) => username.username,
                _ => continue, // todo: possibly add some logging here
            };

            db.pub_key_lookup.insert(owner, username).unwrap();
        }

        if have_updates {
            self.events.meta_changed(Actor::Sync);
        }

        Ok(())
    }

    /// Updates remote and base metadata to local.
    async fn push_meta(&self) -> LbResult<()> {
        let mut updates = vec![];
        let mut local_changes_no_digests = Vec::new();

        let tx = self.ro_tx().await;
        let db = tx.db();

        // remote = local
        let local = db.base_metadata.stage(&db.local_metadata).to_lazy();

        for id in local.tree.staged.ids() {
            let mut local_change = local.tree.staged.find(&id)?.timestamped_value.value.clone();
            let maybe_base_file = local.tree.base.maybe_find(&id);

            // change everything but document hmac and re-sign
            local_change.set_hmac_and_size(
                maybe_base_file.and_then(|f| f.document_hmac().copied()),
                maybe_base_file.and_then(|f| *f.timestamped_value.value.doc_size()),
            );
            let local_change = local_change.sign(&self.keychain)?;

            local_changes_no_digests.push(local_change.clone());
            let file_diff = FileDiff { old: maybe_base_file.cloned(), new: local_change };
            updates.push(file_diff);
        }

        drop(tx);

        if !updates.is_empty() {
            self.client
                .request(self.get_account()?, UpsertRequestV2 { updates: updates.clone() })
                .await?;
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
    // todo: make this so that all document updates are attempted and we don't just return the
    // first error. Once an attempt is made we can return any or all errors, either would be an
    // improvement
    async fn push_docs(&self) -> LbResult<()> {
        let mut updates = vec![];
        let mut local_changes_digests_only = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();
        let start = Instant::now();

        let local = db.base_metadata.stage(&db.local_metadata).to_lazy();

        for id in local.tree.staged.ids() {
            let base_file = local.tree.base.find(&id)?.clone();

            // change only document hmac and re-sign
            let mut local_change = base_file.timestamped_value.value.clone();
            local_change.set_hmac_and_size(
                local.find(&id)?.document_hmac().copied(),
                *local.find(&id)?.timestamped_value.value.doc_size(),
            );

            if base_file.document_hmac() == local_change.document_hmac()
                || local_change.document_hmac().is_none()
            {
                continue;
            }

            let local_change = local_change.sign(&self.keychain)?;

            updates.push(FileDiff { old: Some(base_file), new: local_change.clone() });
            local_changes_digests_only.push(local_change);
            self.events
                .sync_update(SyncIncrement::PushingDocument(id, true));
        }

        drop(tx);
        if start.elapsed() > web_time::Duration::from_millis(100) {
            warn!("sync push_docs held lock for {:?}", start.elapsed());
        }

        let futures = updates.clone().into_iter().map(|diff| self.push_doc(diff));

        let mut stream = stream::iter(futures).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        while let Some(fut) = stream.next().await {
            let id = fut?;
            self.events
                .sync_update(SyncIncrement::PushingDocument(id, false));
        }

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

    async fn push_doc(&self, diff: FileDiff<SignedMeta>) -> LbResult<Uuid> {
        let id = *diff.new.id();
        let hmac = diff.new.document_hmac();
        let local_document_change = self.docs.get(id, hmac.copied()).await?;
        self.client
            .request(
                self.get_account()?,
                ChangeDocRequestV2 { diff, new_content: local_document_change },
            )
            .await?;

        Ok(id)
    }

    async fn send_debug_info(self, account: Account) {
        tokio::spawn(async move {
            self.client
                .request(
                    &account,
                    UpsertDebugInfoRequest {
                        debug_info: self
                            .debug_info("none provided - sync".to_string(), false)
                            .await
                            .unwrap(),
                    },
                )
                .await
                .unwrap();
        });
    }

    async fn read_document_helper<T>(
        &self, id: Uuid, tree: &mut LazyTree<T>,
    ) -> LbResult<DecryptedDocument>
    where
        T: TreeLike<F = SignedMeta>,
    {
        let file = tree.find(&id)?;
        validate::is_document(file)?;
        let hmac = file.document_hmac().copied();

        if tree.calculate_deleted(&id)? {
            return Err(LbErrKind::FileNonexistent.into());
        }

        let doc = match hmac {
            Some(hmac) => {
                let doc = self.docs.get(id, Some(hmac)).await?;
                tree.decrypt_document(&id, &doc, &self.keychain)?
            }
            None => vec![],
        };

        Ok(doc)
    }

    /// for tests only
    #[doc(hidden)]
    pub async fn server_dirty_ids(&self) -> LbResult<Vec<Uuid>> {
        let mut state = self.syncer.lock().await;
        self.inital_sync_state(&mut state).await?;
        self.process_deletions().await?;
        self.fetch_meta(&mut state).await?;

        let server_ids = state.remote_changes.iter().map(|f| *f.id()).collect();

        Ok(server_ids)
    }

    pub(crate) fn setup_syncer(&self) {
        if self.config.background_work {
            self.clone().local_change_worker();
            self.clone().periodic_sync_worker();
            self.clone().post_sync_worker();
        }
    }

    fn local_change_worker(self) {
        tokio::spawn(async move {
            let mut events = self.subscribe();

            let sync_criteria = |e: Event| {
                matches!(
                    e,
                    Event::MetadataChanged(Actor::User) | Event::DocumentWritten(_, Actor::User)
                )
            };

            loop {
                let mut should_sync = false;

                // drain the current channel, so we don't sync for each keystroke if they pile up
                loop {
                    let event = events.try_recv();
                    match event {
                        Ok(event) => {
                            if sync_criteria(event) {
                                should_sync = true;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        _ => {
                            panic!(
                                "unexpected broadcast receive error, returning local_change_worker"
                            );
                        }
                    }
                }

                // empty channel + nothing interesting has happened, sit and wait for something
                // interesting
                if !should_sync {
                    let event = events.recv().await.unwrap();
                    if sync_criteria(event) {
                        self.sync().await.map_unexpected().log_and_ignore();
                    } else {
                        continue;
                    }
                }
            }
        });
    }

    fn periodic_sync_worker(self) {
        tokio::spawn(async move {
            self.sync().await.map_unexpected().log_and_ignore();
            if self.user_active().await {
                tokio::time::sleep(Duration::from_secs(3)).await;
            } else {
                tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            }
        });
    }

    async fn user_active(&self) -> bool {
        let last_seen = self.user_last_seen.read().await;
        last_seen.elapsed() < Duration::from_secs(3 * 60)
    }

    fn post_sync_worker(self) {
        tokio::spawn(async move {
            let mut events = self.subscribe();

            loop {
                let event = events.recv().await.unwrap();
                if let Event::Sync(SyncIncrement::SyncFinished(_)) = event {
                    self.fetcher().await.map_unexpected().log_and_ignore();
                    self.populate_pk_cache()
                        .await
                        .map_unexpected()
                        .log_and_ignore();
                };
            }
        });
    }

    async fn fetcher(&self) -> LbResult<()> {
        let mut files_to_pull = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();

        let Some(root) = db.root.get() else {
            return Ok(());
        };

        // we can only fetch things we know the server knows about
        let mut tree = db.base_metadata.stage(None).to_lazy();

        for id in tree.descendants_using_links(root)? {
            let file = tree.find(&id)?;
            let hmac = file.document_hmac().copied();

            // skip non-documents
            if !file.is_document() {
                continue;
            }

            // skip deleted files
            if tree.calculate_deleted(&id)? {
                continue;
            }

            // skip non-first-party files
            let name = tree.name(&id, &self.keychain)?;
            if !name.ends_with(".md") && !name.ends_with(".svg") {
                continue;
            }

            files_to_pull.push((id, hmac));
        }

        drop(tx);

        // this could all be done in parallel, but for now going to not do it that way
        // benefits: less work, but also ensures that a file that needs to be fetched immediately
        // can be
        for (id, hmac) in files_to_pull {
            if let Some(hmac) = hmac {
                self.fetch_doc(id, hmac).await?;
            }
        }

        Ok(())
    }
}
