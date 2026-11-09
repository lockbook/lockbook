use crate::io::server::Server;
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[cfg(not(any(target_family = "wasm", target_os = "ios")))]
use std::fs::{File, TryLockError};

use futures::{StreamExt, stream};
use tokio::sync::Mutex;
#[cfg(not(target_family = "wasm"))]
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::time;
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
        crypto::{AESKey, DecryptedDocument, EncryptedDocument},
        errors::{LbErr, Unexpected},
        file::ShareMode,
        file_like::FileLike,
        file_metadata::{DocumentHmac, FileDiff, FileType, Owner},
        filename::{DocumentType, NameComponents},
        lazy::LazyTree,
        signed_meta::SignedMeta,
        staged::StagedTreeLikeMut,
        symkey,
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

impl<S: Server> Lb<S> {
    /// Outside iOS and WASM, returns `AlreadySyncing` if another instance is syncing this data directory.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn sync(&self) -> LbResult<()> {
        let mut sync_state = self.syncer.lock().await;
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
        let _sync_lock = self.lock_sync()?;
        self.events.sync_update(SyncIncrement::SyncStarted);

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

        #[cfg(not(target_family = "wasm"))]
        if account.is_beta() {
            self.send_debug_info(account).await;
        }

        Ok(())
    }

    #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
    fn lock_sync(&self) -> LbResult<Option<File>> {
        let Some(path) = &self.sync_lock else {
            return Ok(None);
        };
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(TryLockError::WouldBlock) => Err(LbErrKind::AlreadySyncing.into()),
            Err(TryLockError::Error(err)) => Err(err.into()),
        }
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
        // Catch up after acquiring the sync lock, before using the previous sync's cursor.
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        *state = Default::default();
        state.last_synced = db.last_synced.as_ref().copied().unwrap_or_default() as u64;
        tx.end();

        Ok(())
    }

    pub(crate) async fn process_deletions(&self) -> LbResult<()> {
        let server_ids = self
            .client
            .get_file_ids(self.get_account()?, GetFileIdsRequest {})
            .await?
            .ids;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut local = db.base_metadata.stage(&db.local_metadata).to_lazy();
        let base_ids = local.tree.base.ids();

        // files the server stopped listing, and this device's files under them it never had
        let mut prunable_ids: HashSet<Uuid> = base_ids
            .into_iter()
            .filter(|id| !server_ids.contains(id))
            .collect();
        let mut to_visit: Vec<Uuid> = prunable_ids.iter().copied().collect();
        while let Some(id) = to_visit.pop() {
            for child in local.children(&id)? {
                if !server_ids.contains(&child) && prunable_ids.insert(child) {
                    to_visit.push(child);
                }
            }
        }

        // a file the server still has keeps its other changes: a pending move into a pruned
        // folder is undone
        let mut moved_in = Vec::new();
        for id in db.local_metadata.ids() {
            let parent = *local.find(&id)?.parent();
            if let Some(base_file) = local.tree.base.maybe_find(&id) {
                if prunable_ids.contains(&parent)
                    && !prunable_ids.contains(&id)
                    && !prunable_ids.contains(base_file.parent())
                {
                    moved_in.push((id, *base_file.parent()));
                }
            }
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

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        for (id, parent) in moved_in {
            tree.move_unvalidated(&id, &parent, &self.keychain)?;
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
            .get_updates_v2(
                self.get_account()?,
                GetUpdatesRequestV2 { since_metadata_version: state.last_synced },
            )
            .await?;

        let tx = self.ro_tx().await;
        let db = tx.db();

        // this loop implicitly prunes remote orphans; like validation, a deleted file whose
        // parent left the tree is not one
        let mut without_orphans = Vec::new();
        let me = Owner(self.keychain.get_pk()?);
        let remote = db.base_metadata.stage(updates.file_metadata).to_lazy();
        for id in remote.tree.staged.ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || meta.explicitly_deleted()
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
        if db.root.as_ref().is_none() {
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
            .map(|(id, hmac)| async move { self.ensure_doc_available(id, hmac).await.map(|_| id) });

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

    pub(crate) async fn ensure_doc_available(
        &self, id: Uuid, hmac: DocumentHmac,
    ) -> LbResult<Option<EncryptedDocument>> {
        // todo: in a lot of cases there is a list of ids we're trying to get, it would be better
        // if the caller managed the event updates, the status would be more meaningful for longer

        // in this world, can we get stuck pushing a doc as well? Probably fine for now
        if self.docs.exists(id, Some(hmac)) {
            return Ok(None);
        }

        self.events
            .sync_update(SyncIncrement::PullingDocument(id, true));
        let remote_document = self
            .client
            .get_document(self.get_account()?, GetDocRequest { id, hmac })
            .await?;
        self.docs
            .insert(id, Some(hmac), &remote_document.content)
            .await?;
        self.events
            .sync_update(SyncIncrement::PullingDocument(id, false));

        Ok(Some(remote_document.content))
    }

    pub(crate) async fn fetch_doc(
        &self, id: Uuid, hmac: DocumentHmac,
    ) -> LbResult<EncryptedDocument> {
        match self.ensure_doc_available(id, hmac).await? {
            Some(doc) => Ok(doc),
            None => self.docs.get(id, Some(hmac)).await,
        }
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
            // Id and key share one lifetime: the merge, including construction retries.
            // A new key per attempt encrypts with the key cached by the failed attempt.
            let mut duplicate_file_ids: HashMap<Uuid, (Uuid, AESKey)> = HashMap::new();

            'merge_construction: loop {
                // process just the edits which allow us to check deletions in the result
                let mut remote_with_moves = {
                    let mut remote_with_moves = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations
                    let mut deletion_creations = HashSet::new();
                    for id in db.local_metadata.ids() {
                        if remote.maybe_find(&id).is_none() && !links_to_delete.contains(&id) {
                            deletion_creations.insert(id);
                        }
                    }
                    for id in parents_first(&local, &deletion_creations)? {
                        let local_file = local.find(&id)?.clone();
                        remote_with_moves
                            .create_unvalidated(
                                id,
                                symkey::generate_key(),
                                local_file.parent(),
                                &local.name(&id, &self.keychain)?,
                                local_file.file_type(),
                                &self.keychain,
                            )
                            .map_err(orphaned_creation(id))?;
                    }

                    // moves (creations happen first in case a file is moved into a new folder);
                    // as in the merge, skipping files the server moved or deleted
                    for id in db.local_metadata.ids() {
                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = db.base_metadata.maybe_find(&id).cloned() {
                            if !local_file.explicitly_deleted()
                                && local_file.parent() != base_file.parent()
                                && remote.find(&id)?.parent() == base_file.parent()
                                && !remote.calculate_deleted(&id)?
                                && !files_to_unmove.contains(&id)
                            {
                                // a move this device may no longer make is undone; at the source
                                // only the server's grant counts, since leaving a share after
                                // moving out of it is this device's own later change
                                let from = remote.find(&id)?.parent();
                                let may_move_from = remote.maybe_find(from).is_some()
                                    && remote.access_mode(me, from)? >= Some(UserAccessMode::Write);
                                if !may_move_from
                                    || !may_write_in(&local, &remote, me, local_file.parent())?
                                {
                                    files_to_unmove.insert(id);
                                    continue;
                                }
                                // move
                                remote_with_moves.move_unvalidated(
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
                        let parent = *remote.maybe_find(&id).unwrap_or(&local_file).parent();
                        if local_file.explicitly_deleted()
                            && !lost_access(&local, me, &id)?
                            && may_write_in(&local, &remote, me, &parent)?
                        {
                            // delete
                            remote_with_moves.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }
                    remote_with_moves
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                let mut merge = {
                    let mut merge = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations and edits of created documents
                    let mut creations = HashSet::new();
                    for id in db.local_metadata.ids() {
                        if remote_with_moves.maybe_find(&id).is_some()
                            && !remote_with_moves.calculate_deleted(&id)?
                            && remote.maybe_find(&id).is_none()
                            && !links_to_delete.contains(&id)
                            && !lost_access(&local, me, &id)?
                            && may_write_in(
                                &local,
                                &remote_with_moves,
                                me,
                                local.find(&id)?.parent(),
                            )?
                        {
                            creations.insert(id);
                        }
                    }
                    for id in parents_first(&local, &creations)? {
                        let local_file = local.find(&id)?.clone();
                        merge
                            .create_unvalidated(
                                id,
                                local.decrypt_key(&id, &self.keychain)?,
                                local_file.parent(),
                                &local.name(&id, &self.keychain)?,
                                local_file.file_type(),
                                &self.keychain,
                            )
                            .map_err(orphaned_creation(id))?;
                    }

                    // deletes
                    // before moves, so a moved folder leaves the files this device deleted as they
                    // were; a file is deleted itself unless its parent is deleted and holds it in
                    // the merge too, since moves skip deleted files but may lift live ones
                    for id in db.local_metadata.ids() {
                        if db.base_metadata.maybe_find(&id).is_none()
                            || !remote_with_moves.calculate_deleted(&id)?
                            || remote.calculate_deleted(&id)?
                        {
                            continue;
                        }
                        let deleted = remote_with_moves.find(&id)?;
                        if deleted.explicitly_deleted()
                            || deleted.parent() != merge.find(&id)?.parent()
                        {
                            // delete
                            merge.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }
                    for &id in &links_to_delete {
                        // delete
                        if merge.maybe_find(&id).is_some()
                            && !merge.find(&id)?.explicitly_deleted()
                            && !(remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                        {
                            merge.delete_unvalidated(&id, &self.keychain)?;
                        }
                    }

                    // skip files that are already deleted, will be deleted, were not created, or
                    // are no longer reachable from this device
                    let mut replayed = Vec::new();
                    for id in db.local_metadata.ids() {
                        if merge.maybe_find(&id).is_some()
                            && remote_with_moves.maybe_find(&id).is_some()
                            && !remote_with_moves.calculate_deleted(&id)?
                            && !(remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                            && !links_to_delete.contains(&id)
                            && !lost_access(&local, me, &id)?
                        {
                            replayed.push(id);
                        }
                    }

                    // moves (creations happen first in case a file is moved into a new folder)
                    for &id in &replayed {
                        let local_file = local.find(&id)?;
                        if let Some(base_file) = base.maybe_find(&id) {
                            if local_file.parent() != base_file.parent()
                                && remote.find(&id)?.parent() == base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                let parent = *local_file.parent();
                                merge.move_unvalidated(&id, &parent, &self.keychain)?;
                            }
                        }
                    }
                    // access is read from the merge below, which needs a tree without cycles; the
                    // full validation after construction reports every other failure
                    if let Err(LbErr {
                        kind: LbErrKind::Validation(ValidationFailure::Cycle(ids)),
                        ..
                    }) = merge.assert_no_cycles()
                    {
                        unmove_cycle(&db.local_metadata, &ids, &mut files_to_unmove)?;
                        continue 'merge_construction;
                    }

                    // renames, edits, and shares (moves happen first so each file is where
                    // the merge puts it)
                    for id in replayed {
                        let local_file = local.find(&id)?.clone();
                        let local_name = local.name(&id, &self.keychain)?;
                        let maybe_base_file = base.maybe_find(&id).cloned();
                        let maybe_remote_file = remote.maybe_find(&id).cloned();
                        if maybe_base_file.is_some() {
                            let base_name = base.name(&id, &self.keychain)?;
                            let remote_name = remote.name(&id, &self.keychain)?;

                            // rename
                            if local_name != base_name
                                && remote_name == base_name
                                && may_write_in(
                                    &local,
                                    &remote_with_moves,
                                    me,
                                    merge.find(&id)?.parent(),
                                )?
                            {
                                merge.rename_unvalidated(&id, &local_name, &self.keychain)?;
                            }
                        }

                        // share: replay what this device changed about each recipient's share;
                        // when the server changed it too, this device wins if either side unshared,
                        // otherwise the higher mode
                        let writable = may_write_in(&local, &remote_with_moves, me, &id)?;
                        let base_keys = shares_by_recipient(maybe_base_file.as_ref());
                        let remote_keys = shares_by_recipient(maybe_remote_file.as_ref());
                        let owner = merge.find(&id)?.owner();
                        for (for_, local) in shares_by_recipient(Some(&local_file)) {
                            let base = base_keys.get(&for_).copied();
                            // a move may have made the recipient the owner
                            if Some(local) == base || for_ == owner {
                                continue;
                            }
                            let remote = remote_keys.get(&for_).copied();
                            let remote_live = remote.filter(|&(_, deleted)| !deleted);
                            let (mode, deleted) = local;
                            if deleted {
                                let mine = for_ == me;
                                if remote_live.is_some() && (mine || writable) {
                                    merge.delete_share_unvalidated(
                                        &id,
                                        Some(for_.0),
                                        &self.keychain,
                                    )?;
                                }
                                continue;
                            }
                            let wins = remote == base
                                || remote_live.is_none_or(|(remote_mode, _)| mode > remote_mode);
                            let mode = match mode {
                                UserAccessMode::Read => ShareMode::Read,
                                UserAccessMode::Write => ShareMode::Write,
                                UserAccessMode::Owner => continue,
                            };
                            if wins && writable {
                                merge.add_share_unvalidated(id, for_, mode, &self.keychain)?;
                            }
                        }
                        // share deletion due to conflicts
                        if files_to_unshare.contains(&id) {
                            merge.delete_share_unvalidated(&id, None, &self.keychain)?;
                        }

                        // rename due to path conflict
                        if let Some(&rename_increment) = rename_increments.get(&id) {
                            let name = if local_file.is_folder() {
                                NameComponents::folder(&local_name)
                            } else {
                                NameComponents::from(&local_name)
                            };
                            let name = name.numbered(rename_increment + 1);
                            merge.rename_unvalidated(&id, &name, &self.keychain)?;
                        }

                        // edit
                        let base_hmac = maybe_base_file.and_then(|f| f.document_hmac().cloned());
                        let remote_hmac =
                            maybe_remote_file.and_then(|f| f.document_hmac().cloned());
                        let local_hmac = local_file.document_hmac().cloned();
                        if may_write_in(&local, &remote_with_moves, me, &id)?
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

                                match document_type.merge(
                                    &base_document,
                                    &local_document,
                                    &remote_document,
                                ) {
                                    Some(merged_document) => {
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document,
                                                &self.keychain,
                                            )?;
                                        let hmac = merge.find(&id)?.document_hmac().copied();
                                        self.docs.insert(id, hmac, &encrypted_document).await?;
                                    }
                                    None => {
                                        // duplicate file, in the document's folder if this device
                                        // may write there, otherwise beside its link; with no
                                        // link there's nowhere to put it
                                        let parent = *merge.find(&id)?.parent();
                                        let writable = merge.maybe_find(&parent).is_some()
                                            && may_write_in(
                                                &local,
                                                &remote_with_moves,
                                                me,
                                                &parent,
                                            )?;
                                        let copy_parent = if writable {
                                            Some(parent)
                                        } else if let Some(link) = merge.linked_by(&id)? {
                                            Some(*merge.find(&link)?.parent())
                                        } else {
                                            None
                                        };
                                        if let Some(merge_parent) = copy_parent {
                                            let (duplicate_id, duplicate_key) =
                                                if let Some(&tracked) = duplicate_file_ids.get(&id)
                                                {
                                                    tracked
                                                } else {
                                                    let duplicate_id = Uuid::new_v4();
                                                    let duplicate_key = symkey::generate_key();
                                                    duplicate_file_ids
                                                        .insert(id, (duplicate_id, duplicate_key));
                                                    rename_increments.insert(duplicate_id, 1);
                                                    (duplicate_id, duplicate_key)
                                                };

                                            let merge_name = NameComponents::from(&merge_name)
                                                .copy(rename_increments[&duplicate_id]);

                                            merge.create_unvalidated(
                                                duplicate_id,
                                                duplicate_key,
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
                                }
                            } else {
                                let local_file = local.find(&id)?;
                                merge.overwrite_document_hmac_unvalidated(
                                    &id,
                                    local_file.document_hmac().copied(),
                                    local_file.doc_size(),
                                    &self.keychain,
                                )?;
                            }
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
                                unmove_cycle(&db.local_metadata, ids, &mut files_to_unmove)?;
                            }
                            ValidationFailure::PathConflict(ids) => {
                                // pick one local id and generate a non-conflicting filename
                                let mut progress = false;
                                for &id in ids {
                                    if duplicate_file_ids.values().any(|&(dup, _)| dup == id) {
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
                                // if ancestor is newly shared, delete share; if this device moved
                                // the link, or a folder between it and the share, into the share,
                                // undo that move unless it would land in a deleted folder;
                                // otherwise delete link
                                let mut progress = false;
                                if let Some(base_shared_ancestor) = base.maybe_find(shared_ancestor)
                                {
                                    if !base_shared_ancestor.is_shared()
                                        && files_to_unshare.insert(*shared_ancestor)
                                    {
                                        progress = true;
                                    }
                                }
                                let mut id = *link;
                                while !progress && id != *shared_ancestor {
                                    if let (Some(base_file), Some(remote_file)) =
                                        (base.maybe_find(&id), remote.maybe_find(&id))
                                    {
                                        let from = *remote_file.parent();
                                        let moved_here =
                                            db.local_metadata.maybe_find(&id).is_some()
                                                && local.find(&id)?.parent() != base_file.parent()
                                                && from == *base_file.parent();
                                        if moved_here
                                            && !remote_with_moves.calculate_deleted(&from)?
                                            && files_to_unmove.insert(id)
                                        {
                                            progress = true;
                                        }
                                    }
                                    id = *merge.find(&id)?.parent();
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
                            | ValidationFailure::NonDecryptableFileName(_)
                            | ValidationFailure::SelfShare(_) => {
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
        if state.new_root.is_some() {
            self.events.signed_in();
        }

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
        db.last_synced.replace(state.updates_as_of as i64)?;

        if let Some(root) = state.new_root {
            db.root.replace(root)?;
        }

        Ok(())
    }

    async fn populate_pk_cache(&self) -> LbResult<()> {
        // todo: is this the move?
        let mut missing_owners = HashSet::new();
        {
            let tx = self.ro_tx().await;
            let db = tx.db();
            for file in db.base_metadata.iter().map(|(_, value)| value) {
                for user_access_key in file.user_access_keys() {
                    let enc_by = Owner(user_access_key.encrypted_by);
                    let enc_for = Owner(user_access_key.encrypted_for);

                    if !db.pub_key_lookup.contains_key(&enc_by) {
                        missing_owners.insert(enc_by);
                    }

                    if !db.pub_key_lookup.contains_key(&enc_for) {
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
                    .get_username(self.get_account().unwrap(), GetUsernameRequest { key: owner.0 })
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
                .upsert_file_metadata_v2(
                    self.get_account()?,
                    UpsertRequestV2 { updates: updates.clone() },
                )
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

        let mut docs_without_errors = vec![];
        let mut last_error: Option<LbErr> = None;

        while let Some(fut) = stream.next().await {
            match fut {
                Ok(id) => {
                    docs_without_errors.push(id);
                    self.events
                        .sync_update(SyncIncrement::PushingDocument(id, false));
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        local_changes_digests_only.retain(|f| docs_without_errors.contains(f.id()));

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        // base = local (metadata)
        (&mut db.base_metadata)
            .to_lazy()
            .stage(local_changes_digests_only)
            .promote()?;

        db.base_metadata.stage(&mut db.local_metadata).prune()?;

        tx.end();

        if let Some(err) = last_error { Err(err) } else { Ok(()) }
    }

    async fn push_doc(&self, diff: FileDiff<SignedMeta>) -> LbResult<Uuid> {
        let id = *diff.new.id();
        let hmac = diff.new.document_hmac();
        let local_document_change = self.docs.get(id, hmac.copied()).await?;
        self.client
            .change_doc_v2(
                self.get_account()?,
                ChangeDocRequestV2 { diff, new_content: local_document_change },
            )
            .await?;

        Ok(id)
    }

    #[cfg(not(target_family = "wasm"))]
    async fn send_debug_info(&self, account: Account) {
        use crate::service::debug;

        let max_panic_time = debug::latest_panic_time(&self.config.writeable_path)
            .await
            .unwrap_or_else(|e| {
                warn!("could not enumerate panic files: {e:?}");
                None
            });

        let last_sent = {
            let tx = self.ro_tx().await;
            tx.db().last_extracted_panic.as_ref().copied()
        };

        let should_send = match (last_sent, max_panic_time) {
            (None, _) => true,
            (Some(prev), Some(cur)) => cur > prev,
            (Some(_), None) => false,
        };

        if !should_send {
            return;
        }

        let debug_info = match self.debug_info(false).await {
            Ok(debug_info) => debug_info,
            Err(e) => {
                warn!("could not collect debug_info: {e:?}");
                return;
            }
        };

        let new_marker = max_panic_time.unwrap_or(0);
        let bg_self = self.clone();
        let task = async move {
            match bg_self
                .client
                .upsert_debug_info(&account, UpsertDebugInfoRequest { debug_info })
                .await
            {
                Ok(_) => {
                    let mut tx = bg_self.begin_tx().await;
                    if let Err(e) = tx.db().last_extracted_panic.replace(new_marker) {
                        warn!("could not record last_extracted_panic: {e:?}");
                    }
                    tx.end();
                }
                Err(e) => warn!("send_debug_info failed: {e:?}"),
            }
        };

        if self.config.background_work {
            tokio::spawn(task);
        } else {
            task.await;
        }
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
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
        let _sync_lock = self.lock_sync()?;
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
        #[cfg(not(target_family = "wasm"))]
        tokio::spawn(async move {
            let mut events = self.subscribe();

            let sync_criteria = |e: Event| {
                matches!(
                    e,
                    Event::MetadataChanged(Actor::User(_))
                        | Event::DocumentWritten(_, Actor::User(_))
                )
            };

            loop {
                time::sleep(Duration::from_millis(500)).await;
                let mut should_sync = false;

                // drain the current channel, so we don't sync for each keystroke if they pile up
                loop {
                    match events.try_recv() {
                        Ok(event) => should_sync |= sync_criteria(event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Lagged(_)) => should_sync |= self.has_local_work().await,
                        Err(TryRecvError::Closed) => return,
                    }
                }

                // empty channel + nothing interesting has happened, sit and wait for something
                // interesting
                if !should_sync {
                    should_sync = match events.recv().await {
                        Ok(event) => sync_criteria(event),
                        Err(RecvError::Lagged(_)) => self.has_local_work().await,
                        Err(RecvError::Closed) => return,
                    };
                }

                if should_sync {
                    self.sync().await.map_unexpected().log_and_ignore();
                }
            }
        });
    }

    /// Whether a sync would push anything: what a missed event changed is still unsynced.
    #[cfg(not(target_family = "wasm"))]
    async fn has_local_work(&self) -> bool {
        !self.local_changes().await.is_empty()
    }

    fn periodic_sync_worker(self) {
        #[cfg(not(target_family = "wasm"))]
        tokio::spawn(async move {
            loop {
                if self.user_active().await {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                } else {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(5 * 60)) => {}
                        _ = self.user_wake.notified() => {}
                    }
                }
                self.sync().await.map_unexpected().log_and_ignore();
            }
        });
    }

    pub(crate) async fn user_active(&self) -> bool {
        let last_seen = self.user_last_seen.read().await;
        last_seen.elapsed() < Duration::from_secs(15)
    }

    fn post_sync_worker(self) {
        #[cfg(not(target_family = "wasm"))]
        tokio::spawn(async move {
            let mut events = self.subscribe();

            loop {
                // A receiver that lagged missed a sync's worth of events, a `SyncFinished`
                // possibly among them; the work after a sync is idempotent, so it runs again.
                match events.recv().await {
                    Ok(Event::Sync(SyncIncrement::SyncFinished(_))) | Err(RecvError::Lagged(_)) => {
                        self.fetcher().await.map_unexpected().log_and_ignore();
                        self.populate_pk_cache()
                            .await
                            .map_unexpected()
                            .log_and_ignore();
                    }
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }

    async fn fetcher(&self) -> LbResult<()> {
        let mut files_to_pull = vec![];

        let tx = self.ro_tx().await;
        let db = tx.db();

        let Some(root) = db.root.as_ref() else {
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

            if self.docs.exists(id, hmac) {
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
                self.ensure_doc_available(id, hmac).await?;
            }
        }

        Ok(())
    }
}

/// Whether this device may still change `id` or what's in it: write is granted at the nearest file
/// `remote` has, at or above `id` in this device's tree, by a share this device hasn't declined,
/// and in this device's own tree. `remote` is the server's tree, with this device's kept moves
/// applied once they are, so shares are read where each file ends up, not the merge's.
fn may_write_in<L: TreeLike, R: TreeLike<F = L::F>>(
    local: &LazyTree<L>, remote: &LazyTree<R>, me: Owner, id: &Uuid,
) -> LbResult<bool> {
    if local.maybe_find(id).is_some() && local.access_mode(me, id)? < Some(UserAccessMode::Write) {
        return Ok(false);
    }
    let mut id = *id;
    while remote.maybe_find(&id).is_none() {
        let Some(file) = local.maybe_find(&id) else {
            return Ok(false);
        };
        if file.parent() == file.id() {
            return Ok(false);
        }
        id = *file.parent();
    }
    // the server's access at `id`, less the shares this device declined
    let mut ids = remote.ancestors(&id)?;
    ids.insert(id);
    let mut mode = None;
    for id in ids {
        let declined = local.maybe_find(&id).is_some_and(|file| {
            file.user_access_keys()
                .iter()
                .any(|key| key.encrypted_for == me.0 && key.deleted)
        });
        if !declined {
            mode = mode.max(remote.find(&id)?.access_mode(&me));
        }
    }
    Ok(mode >= Some(UserAccessMode::Write))
}

/// Undoes this device's moves that form a cycle, or fails when none of them is this device's.
fn unmove_cycle<T: TreeLike>(
    local_changes: &T, ids: &HashSet<Uuid>, files_to_unmove: &mut HashSet<Uuid>,
) -> LbResult<()> {
    let mut progress = false;
    for &id in ids {
        if local_changes.maybe_find(&id).is_some() && files_to_unmove.insert(id) {
            progress = true;
        }
    }
    if !progress {
        return Err(LbErrKind::Unexpected(format!("sync failed to resolve cycle: {ids:?}")).into());
    }
    Ok(())
}

/// `ids` ordered so each file comes after its parent when that parent is also in `ids`.
fn parents_first<T: TreeLike>(tree: &LazyTree<T>, ids: &HashSet<Uuid>) -> LbResult<Vec<Uuid>> {
    let mut order = Vec::with_capacity(ids.len());
    let mut placed = HashSet::with_capacity(ids.len());
    for &id in ids {
        let mut unplaced_ancestry = Vec::new();
        let mut id = id;
        while ids.contains(&id) && !placed.contains(&id) && !unplaced_ancestry.contains(&id) {
            unplaced_ancestry.push(id);
            id = *tree.find(&id)?.parent();
        }
        for id in unplaced_ancestry.into_iter().rev() {
            placed.insert(id);
            order.push(id);
        }
    }
    Ok(order)
}

/// A creation whose parent is in neither the tree nor the creations before it.
fn orphaned_creation(id: Uuid) -> impl FnOnce(LbErr) -> LbErr {
    move |err| match err.kind {
        LbErrKind::FileParentNonexistent => LbErrKind::Unexpected(format!(
            "sync failed to find a topological order for file creation {id}"
        ))
        .into(),
        _ => err,
    }
}

/// A file this device has no access to, e.g. inside a share it declined. A share root carrying
/// this device's own (possibly tombstoned) key is not lost: that tombstone is what gets pushed.
fn lost_access<T: TreeLike>(tree: &LazyTree<T>, me: Owner, id: &Uuid) -> LbResult<bool> {
    let is_my_share_root = tree
        .find(id)?
        .user_access_keys()
        .iter()
        .any(|k| k.encrypted_for == me.0);
    let reachable = tree.access_mode(me, id)?.is_some();
    Ok(!is_my_share_root && !reachable)
}

/// Each recipient's share of a file as (mode, deleted); a file keeps one share per recipient.
fn shares_by_recipient(file: Option<&SignedMeta>) -> HashMap<Owner, (UserAccessMode, bool)> {
    file.into_iter()
        .flat_map(|file| file.user_access_keys())
        .map(|key| (Owner(key.encrypted_for), (key.mode, key.deleted)))
        .collect()
}
