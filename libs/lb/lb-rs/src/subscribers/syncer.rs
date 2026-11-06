use crate::io::server::Server;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(not(any(target_family = "wasm", target_os = "ios")))]
use std::fs::{File, TryLockError};

use tokio::sync::{Mutex, broadcast::error::TryRecvError};
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
            GetDocUpdatesRequest, GetFileIdsRequest, GetUpdatesRequestV2, GetUsernameError,
            GetUsernameRequest, UpsertDebugInfoRequest, UpsertRequestV2,
        },
        errors::Unexpected,
        file::ShareMode,
        file_like::FileLike,
        file_metadata::{FileDiff, FileType, Owner},
        filename::NameComponents,
        lazy::LazyTree,
        signed_meta::SignedMeta,
        staged::StagedTreeLikeMut,
        symkey,
        tree_like::TreeLike,
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
            self.pull_doc_updates().await?;
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

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn sync_docs(&self) -> LbResult<()> {
        let _sync_state = self.syncer.lock().await;
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
        let _sync_lock = self.lock_sync()?;

        let pipeline: LbResult<()> = async {
            self.pull_doc_updates().await?;
            self.push_doc_updates().await?;
            Ok(())
        }
        .await;

        self.cleanup().await?;
        pipeline
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
        self.push_doc_updates().await?;

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

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?);
        }
        let mut base_staged = (&mut db.base_metadata).to_lazy().stage(None);
        base_staged.tree.removed = prunable_ids.iter().copied().collect();
        base_staged.promote()?;

        let mut local_staged = (&mut db.local_metadata).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids.iter().copied().collect();
        local_staged.promote()?;

        for id in &prunable_ids {
            db.doc_info.remove(id)?;
        }

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

        // this loop implicitly prunes remote orphans
        let mut without_orphans = Vec::new();
        let me = Owner(self.keychain.get_pk()?);
        let remote = db.base_metadata.stage(updates.file_metadata).to_lazy();
        for id in remote.tree.staged.ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || remote.maybe_find_attached(meta).is_some()
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
            // a local attachments folder whose document already has one: its files go there
            let mut attachments_to_fold: HashMap<Uuid, Uuid> = HashMap::new();
            let mut rename_increments: HashMap<Uuid, usize> = HashMap::new();

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
                        if local_file.explicitly_deleted() && !lost_access(&local, me, &id)? {
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
                            && !attachments_to_fold.contains_key(&id)
                            && !lost_access(&local, me, &id)?
                        {
                            creations.insert(id);
                        }
                    }
                    let attached: Vec<Uuid> = creations.iter().copied().collect();
                    'drain_creations: while !creations.is_empty() {
                        'choose_a_creation: for id in &creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            let parent = attachments_to_fold
                                .get(local_file.parent())
                                .unwrap_or(local_file.parent());
                            let result = merge.create_unvalidated(
                                id,
                                local.decrypt_key(&id, &self.keychain)?,
                                parent,
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
                    for id in attached {
                        if let Some(of) = local.find(&id)?.attachments_of() {
                            merge.attach_unvalidated(&id, &of.id, &self.keychain)?;
                        }
                    }

                    // moves, renames, edits, and shares
                    // creations happen first in case a file is moved into a new folder
                    for id in db.local_metadata.ids() {
                        // skip files that are already deleted, will be deleted, or are no
                        // longer reachable from this device
                        if deletions.maybe_find(&id).is_none()
                            || deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                            || links_to_delete.contains(&id)
                            || attachments_to_fold.contains_key(&id)
                            || lost_access(&local, me, &id)?
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
                                let parent = attachments_to_fold
                                    .get(local_file.parent())
                                    .unwrap_or(local_file.parent());
                                merge.move_unvalidated(&id, parent, &self.keychain)?;
                            }

                            // rename
                            if local_name != base_name && remote_name == base_name {
                                merge.rename_unvalidated(&id, &local_name, &self.keychain)?;
                            }
                        }

                        // share
                        let mut base_keys = HashMap::new();
                        if let Some(ref base_file) = maybe_base_file {
                            for key in base_file.user_access_keys() {
                                base_keys.insert(
                                    (Owner(key.encrypted_by), Owner(key.encrypted_for)),
                                    (key.mode, key.deleted),
                                );
                            }
                        }
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
                                // upgrade/downgrade/undelete share: replay what changed on this
                                // device; a key with no base is a concurrent add and the higher
                                // mode wins
                                let base_key = base_keys.get(&(by, for_)).copied();
                                let upgraded = key.mode > remote_mode
                                    && base_key.is_none_or(|(base_mode, _)| key.mode > base_mode);
                                let downgraded = !key.deleted
                                    && key.mode < remote_mode
                                    && base_key.is_some_and(|(base_mode, _)| key.mode < base_mode);
                                let undeleted = !key.deleted
                                    && remote_deleted
                                    && base_key.is_some_and(|(_, base_deleted)| base_deleted);
                                if upgraded || downgraded || undeleted {
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
                                    if db.local_metadata.maybe_find(&id).is_some() {
                                        *rename_increments.entry(id).or_insert(0) += 1;
                                        progress = true;
                                        break;
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
                            ValidationFailure::DuplicateAttachments { document } => {
                                // move this device's new folder's files into the existing one
                                let mut progress = false;
                                let folders: Vec<Uuid> = merge
                                    .ids()
                                    .into_iter()
                                    .filter(|id| {
                                        merge
                                            .find(id)
                                            .ok()
                                            .and_then(|f| f.attachments_of())
                                            .is_some_and(|of| of.id == *document)
                                    })
                                    .collect();
                                let mut existing = None;
                                for id in &folders {
                                    if remote.maybe_find(id).is_some()
                                        && !merge.calculate_deleted(id)?
                                    {
                                        existing = Some(id);
                                    }
                                }
                                let new = folders.iter().find(|id| remote.maybe_find(id).is_none());
                                if let (Some(&existing), Some(&new)) = (existing, new) {
                                    if attachments_to_fold.insert(new, existing).is_none() {
                                        progress = true;
                                    }
                                }
                                if !progress {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "sync failed to resolve duplicate attachments: document: {document:?}"
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
                            | ValidationFailure::InvalidAttachmentsFolder(_) => {
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
        let mut local_changes = Vec::new();

        let tx = self.ro_tx().await;
        let db = tx.db();

        // remote = local
        let local = db.base_metadata.stage(&db.local_metadata).to_lazy();

        for id in local.tree.staged.ids() {
            let local_change = local.tree.staged.find(&id)?.clone();
            let maybe_base_file = local.tree.base.maybe_find(&id);

            local_changes.push(local_change.clone());
            updates.push(FileDiff { old: maybe_base_file.cloned(), new: local_change });
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
            .stage(local_changes)
            .promote()?;
        db.base_metadata.stage(&mut db.local_metadata).prune()?;

        tx.end();

        Ok(())
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

    /// for tests only
    #[doc(hidden)]
    pub async fn server_dirty_ids(&self) -> LbResult<Vec<Uuid>> {
        let mut state = self.syncer.lock().await;
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
        let _sync_lock = self.lock_sync()?;
        self.inital_sync_state(&mut state).await?;
        self.process_deletions().await?;
        self.fetch_meta(&mut state).await?;

        let mut server_ids: Vec<Uuid> = state.remote_changes.iter().map(|f| *f.id()).collect();

        let since = {
            let tx = self.ro_tx().await;
            tx.db()
                .last_doc_synced
                .as_ref()
                .copied()
                .unwrap_or_default()
        };
        let doc_updates = self
            .client
            .get_doc_updates(self.get_account()?, GetDocUpdatesRequest { since, ids: vec![] })
            .await?;
        let tx = self.ro_tx().await;
        let db = tx.db();
        for (id, head) in doc_updates.heads {
            let known = db.doc_info.get(&id).and_then(|info| info.base_hmac());
            if known != Some(head.hmac) && !server_ids.contains(&id) {
                server_ids.push(id);
            }
        }

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
                let event = events.recv().await.unwrap();
                if let Event::Sync(SyncIncrement::SyncFinished(_)) = event {
                    self.fetch_doc_contents()
                        .await
                        .map_unexpected()
                        .log_and_ignore();
                    self.populate_pk_cache()
                        .await
                        .map_unexpected()
                        .log_and_ignore();
                };
            }
        });
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
