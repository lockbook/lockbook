use crate::{CoreError, CoreResult, OneKey, RequestContext, Requester};
use lockbook_shared::access_info::UserAccessMode;
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetFileIdsRequest, GetUpdatesRequest, GetUpdatesResponse,
    GetUsernameRequest, UpsertRequest,
};
use lockbook_shared::file::ShareMode;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, FileDiff, Owner};
use lockbook_shared::filename::NameComponents;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::staged::StagedTreeLikeMut;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use lockbook_shared::{document_repo, SharedError, ValidationFailure};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

fn should_pull_document(
    maybe_base_hmac: Option<&DocumentHmac>, maybe_remote_hmac: Option<&DocumentHmac>,
) -> bool {
    match (maybe_base_hmac, maybe_remote_hmac) {
        (_, None) => false,
        (None, _) => true,
        (Some(base_hmac), Some(remote_hmac)) => base_hmac != remote_hmac,
    }
}

enum SyncProgressOperation {
    IncrementTotalWork(usize),
    StartWorkUnit(ClientWorkUnit),
}

impl<Client: Requester> RequestContext<'_, '_, Client> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync<F: Fn(SyncProgress)>(
        &mut self, maybe_update_sync_progress: Option<F>,
    ) -> CoreResult<()> {
        // initialize sync progress: 3 metadata pulls + 1 metadata push + num local doc changes
        // note: num doc changes can change as a result of pull (can make new/changes docs deleted or add new docs from merge conflicts)
        let mut num_doc_changes = 0;
        for (id, local_change) in self.tx.local_metadata.get_all() {
            if let Some(base_file) = self.tx.base_metadata.get(id) {
                if local_change.document_hmac() != base_file.document_hmac() {
                    num_doc_changes += 1;
                }
            } else if local_change.document_hmac().is_some() {
                num_doc_changes += 1;
            }
        }
        let mut sync_progress_total = 4 + num_doc_changes; // 3 metadata pulls + 1 metadata push
        let mut sync_progress = 0;
        let mut update_sync_progress = |op: SyncProgressOperation| match op {
            SyncProgressOperation::IncrementTotalWork(inc) => sync_progress_total += inc,
            SyncProgressOperation::StartWorkUnit(work_unit) => {
                if let Some(ref update_sync_progress) = maybe_update_sync_progress {
                    update_sync_progress(SyncProgress {
                        total: sync_progress_total,
                        progress: sync_progress,
                        current_work_unit: work_unit,
                    })
                }
                sync_progress += 1;
            }
        };

        self.pull(&mut update_sync_progress)?;
        self.push_metadata(&mut update_sync_progress)?;
        self.prune()?;
        self.push_documents(&mut update_sync_progress)?;
        let update_as_of = self.pull(&mut update_sync_progress)?;
        self.tx.last_synced.insert(OneKey {}, update_as_of);
        self.populate_public_key_cache()?;

        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, update_sync_progress: &mut F) -> CoreResult<i64>
    where
        F: FnMut(SyncProgressOperation),
    {
        // fetch metadata updates
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PullMetadata));
        let updates = self.get_updates()?;
        let mut remote_changes = updates.file_metadata;
        let update_as_of = updates.as_of_metadata_version;

        // initialize root if this is the first pull on this device
        if self.tx.root.get(&OneKey {}).is_none() {
            let root = remote_changes
                .all_files()?
                .into_iter()
                .find(|f| f.is_root())
                .ok_or(CoreError::RootNonexistent)?;
            self.tx.root.insert(OneKey {}, *root.id());
        }

        // track work
        {
            let base = (&self.tx.base_metadata)
                .stage(&self.tx.local_metadata)
                .to_lazy();
            let mut num_documents_to_pull = 0;
            for id in remote_changes.owned_ids() {
                let maybe_base_hmac = base.maybe_find(&id).and_then(|f| f.document_hmac());
                let maybe_remote_hmac = remote_changes.find(&id)?.document_hmac();
                if should_pull_document(maybe_base_hmac, maybe_remote_hmac) {
                    num_documents_to_pull += 1;
                }
            }
            update_sync_progress(SyncProgressOperation::IncrementTotalWork(num_documents_to_pull));
        }

        // pre-process changes
        remote_changes = self.prune_remote_orphans(remote_changes)?;

        // fetch document updates and local documents for merge
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let owner = Owner(account.public_key());
        remote_changes = {
            let mut remote = (&self.tx.base_metadata).stage(remote_changes).to_lazy();
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
                    update_sync_progress(SyncProgressOperation::StartWorkUnit(
                        ClientWorkUnit::PullDocument(remote.name(&id, account)?),
                    ));
                    let remote_document = self
                        .client
                        .request(account, GetDocRequest { id, hmac: remote_hmac })?
                        .content;
                    document_repo::insert(self.config, &id, Some(&remote_hmac), &remote_document)?;
                }
            }
            let (_, remote_changes) = remote.unstage();
            remote_changes
        };

        // compute merge changes
        let merge_changes = {
            // assemble trees
            let mut base = (&self.tx.base_metadata).to_lazy();
            let remote = (&self.tx.base_metadata).to_staged(&remote_changes);
            let mut remote_lazy = remote.as_lazy();
            let mut local = (&self.tx.base_metadata)
                .to_staged(&self.tx.local_metadata)
                .to_lazy();

            // changeset constraints - these evolve as we try to assemble changes and face validation failures
            let mut files_to_revert_moves: HashSet<Uuid> = HashSet::new();
            let mut files_to_rename: HashSet<Uuid> = HashSet::new();

            println!(
                "merge conflict resolution; local changes: {:?}",
                self.tx.local_metadata.owned_ids()
            );
            loop {
                // process just the edits which allow us to check deletions in the result
                println!("* processing deletions *");
                let mut deletions = {
                    let mut deletions = remote.stage(Vec::new()).to_lazy();

                    // creations
                    let mut deletion_creations = HashSet::new();
                    for id in self.tx.local_metadata.owned_ids() {
                        if self.tx.base_metadata.maybe_find(&id).is_none() {
                            deletion_creations.insert(id);
                        }
                    }
                    'outer: while !deletion_creations.is_empty() {
                        'inner: for id in &deletion_creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            println!("deletions - create: {:?}", id);
                            let result = deletions.create_unvalidated(
                                id,
                                local_file.parent(),
                                &local.name(&id, account)?,
                                local_file.file_type(),
                                account,
                            );
                            println!("\tdone");
                            match result {
                                Ok(_) => {
                                    deletion_creations.remove(&id);
                                    continue 'outer;
                                }
                                Err(SharedError::FileParentNonexistent) => {
                                    continue 'inner;
                                }
                                Err(_) => {
                                    result?;
                                }
                            }
                        }
                        return Err(CoreError::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            deletion_creations
                        )));
                    }

                    // moves (creations happen first in case a file is moved into a new folder)
                    for id in self.tx.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = self.tx.base_metadata.maybe_find(&id).cloned() {
                            if !local_file.explicitly_deleted()
                                && local_file.parent() != base_file.parent()
                                && !files_to_revert_moves.contains(&id)
                            {
                                // move
                                println!(
                                    "deletions - parent of {:?}: {:?} -> {:?}",
                                    local.name(&id, account)?,
                                    local.name(base_file.parent(), account)?,
                                    local.name(local_file.parent(), account)?,
                                );
                                deletions.move_unvalidated(&id, local_file.parent(), account)?;
                                println!("\tdone");
                            }
                        }
                    }

                    // deletions (moves happen first in case a file is moved into a deleted folder)
                    for id in self.tx.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if local_file.explicitly_deleted() {
                            // delete
                            println!("deletions - delete: {:?}", local.name(&id, account)?);
                            deletions.delete_unvalidated(&id, account)?;
                            println!("\tdone");
                        }
                    }
                    deletions
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                println!("* processing all edits *");
                let mut merge = {
                    let mut merge = remote.stage(Vec::new()).to_lazy();

                    for id in self.tx.local_metadata.owned_ids() {
                        println!("local change: {:?} {:?}", id, local.name(&id, account)?);
                    }

                    println!("\ndeletions.implicit_deleted: {:?}\n", deletions.implicit_deleted);

                    // creations
                    let mut creations = HashSet::new();
                    for id in self.tx.local_metadata.owned_ids() {
                        if !deletions.calculate_deleted(&id)? {
                            if self.tx.base_metadata.maybe_find(&id).is_none() {
                                creations.insert(id);
                            }
                        }
                    }

                    println!("\ndeletions.implicit_deleted: {:?}\n", deletions.implicit_deleted);

                    'outer: while !creations.is_empty() {
                        'inner: for id in &creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            println!("create: {:?}", id);
                            let result = merge.create_unvalidated(
                                id,
                                local_file.parent(),
                                &local.name(&id, account)?,
                                local_file.file_type(),
                                account,
                            );
                            match result {
                                Ok(_) => {
                                    creations.remove(&id);
                                    continue 'outer;
                                }
                                Err(SharedError::FileParentNonexistent) => {
                                    continue 'inner;
                                }
                                Err(_) => {
                                    result?;
                                }
                            }
                        }
                        return Err(CoreError::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            creations
                        )));
                    }

                    // moves, renames, edits, and shares
                    // creations happen first in case a file is moved into a new folder
                    for id in self.tx.local_metadata.owned_ids() {
                        // skip files that are already deleted or will be deleted
                        println!(
                            "{:?} deleted: {:?}",
                            local.name(&id, account)?,
                            deletions.calculate_deleted(&id)?
                        );
                        if deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote_lazy.calculate_deleted(&id)?)
                        {
                            println!(
                                "skipped edits for {:?} because it's deleted on remote or local",
                                local.name(&id, account)?
                            );
                            continue;
                        }

                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = self.tx.base_metadata.maybe_find(&id).cloned() {
                            // move
                            if local_file.parent() != base_file.parent()
                                && !files_to_revert_moves.contains(&id)
                            {
                                println!(
                                    "parent of {:?}: {:?} -> {:?}",
                                    local.name(&id, account)?,
                                    local.name(base_file.parent(), account)?,
                                    local.name(local_file.parent(), account)?,
                                );
                                merge.move_unvalidated(&id, local_file.parent(), account)?;
                                println!("\tdone");
                            }

                            // rename
                            let (local_name, base_name) =
                                (local.name(&id, account)?, base.name(&id, account)?);
                            if local_name != base_name {
                                println!("name {:?} -> {:?}", base_name, local_name);
                                merge.rename_unvalidated(&id, &local_name, account)?;
                                println!("\tdone");
                            }

                            // edit
                            if local_file.document_hmac() != base_file.document_hmac() {
                                // todo: avoid reading/decrypting/encrypting document
                                let document = local.read_document(self.config, &id, account)?;
                                println!("document ? -> {:?}", document);
                                merge.update_document_unvalidated(&id, &document, account)?;
                                println!("\tdone");
                            }

                            // share
                            let mut base_keys = HashMap::new();
                            for key in base_file.user_access_keys() {
                                base_keys.insert(
                                    (Owner(key.encrypted_by), Owner(key.encrypted_for)),
                                    (key.mode, key.deleted),
                                );
                            }
                            for key in local_file.user_access_keys() {
                                let (by, for_) =
                                    (Owner(key.encrypted_by), Owner(key.encrypted_for));
                                if base_keys.contains_key(&(by, for_)) && key.deleted {
                                    println!("delete share");
                                    merge.delete_share_unvalidated(&id, Some(for_.0), account)?;
                                    println!("\tdone");
                                } else {
                                    let mode = match key.mode {
                                        UserAccessMode::Read => ShareMode::Read,
                                        UserAccessMode::Write => ShareMode::Write,
                                        UserAccessMode::Owner => continue,
                                    };
                                    println!("add share");
                                    merge.add_share_unvalidated(id, for_, mode, account)?;
                                    println!("\tdone");
                                }
                            }

                            // rename due to path conflict
                            if files_to_rename.contains(&id) {
                                let name =
                                    NameComponents::from(&local_name).generate_next().to_name();
                                println!("name {:?} -> {:?}", local_name, name);
                                merge.rename_unvalidated(&id, &name, account)?;
                                println!("\tdone");
                            }
                        }
                    }

                    // deletions
                    // moves happen first in case a file is moved into a deleted folder
                    for id in self.tx.local_metadata.owned_ids() {
                        if self.tx.base_metadata.maybe_find(&id).is_some() {
                            println!(
                                "{:?} deleted: {:?}",
                                local.name(&id, account)?,
                                deletions.calculate_deleted(&id)?
                            );
                            if deletions.calculate_deleted(&id)? {
                                // delete
                                println!("delete: {:?}", local.name(&id, account)?);
                                merge.delete_unvalidated(&id, account)?;
                                println!("\tdone");
                            }
                        }
                    }

                    merge
                };

                // validate; handle failures by introducing changeset constraints
                println!("validate");
                let validate_result = merge.validate(owner);
                match validate_result {
                    // merge changeset is valid
                    Ok(_) => {
                        let (_, merge_changes) = merge.unstage();
                        break merge_changes;
                    }
                    Err(SharedError::ValidationFailure(ref vf)) => match vf {
                        // merge changeset has resolvable validation errors and needs modification
                        ValidationFailure::Cycle(ids) => {
                            // revert all local moves in the cycle
                            println!("cycle: {:?}", ids);
                            let mut progress = false;
                            for &id in ids {
                                if self.tx.local_metadata.maybe_find(&id).is_some()
                                    && files_to_revert_moves.insert(id)
                                {
                                    progress = true;
                                }
                            }
                            if !progress {
                                return Err(CoreError::Unexpected(format!(
                                    "sync failed to resolve cycle: {:?}",
                                    ids
                                )));
                            }
                        }
                        ValidationFailure::PathConflict(ids) => {
                            // pick one local id and generate a non-conflicting filename
                            let mut progress = false;
                            for &id in ids {
                                if self.tx.local_metadata.maybe_find(&id).is_some()
                                    && files_to_rename.insert(id)
                                {
                                    progress = true;
                                    break;
                                }
                            }
                            if !progress {
                                return Err(CoreError::Unexpected(format!(
                                    "sync failed to resolve path conflict: {:?}",
                                    ids
                                )));
                            }
                        }
                        ValidationFailure::SharedLink { .. } => {}
                        ValidationFailure::DuplicateLink { .. } => {}
                        ValidationFailure::BrokenLink(_) => {}
                        ValidationFailure::OwnedLink(_) => {}
                        // merge changeset has unexpected validation errors
                        ValidationFailure::Orphan(_)
                        | ValidationFailure::NonFolderWithChildren(_)
                        | ValidationFailure::FileWithDifferentOwnerParent(_)
                        | ValidationFailure::NonDecryptableFileName(_) => {
                            println!(
                                "validate - unexpected validation error: {:?}",
                                validate_result
                            );
                            validate_result?;
                        }
                    },
                    // merge changeset has unexpected errors
                    Err(_) => {
                        println!(
                            "validate - unexpected non-validation error: {:?}",
                            validate_result
                        );
                        validate_result?;
                    }
                }
            }
        };

        // base = remote; local = merge
        {
            let mut remote = (&self.tx.base_metadata)
                .to_staged(&remote_changes)
                .to_lazy();
            let mut merge = (&self.tx.base_metadata).to_staged(&merge_changes).to_lazy();
            println!(
                "remote_changes: {:?}",
                remote.resolve_and_finalize(
                    account,
                    remote_changes.owned_ids().into_iter(),
                    &mut self.tx.username_by_public_key
                )
            );
            println!(
                "merge_changes: {:?}",
                merge.resolve_and_finalize(
                    account,
                    merge_changes.owned_ids().into_iter(),
                    &mut self.tx.username_by_public_key
                )
            );
        }
        (&mut self.tx.base_metadata)
            .to_staged(remote_changes)
            .to_lazy()
            .promote();
        self.tx.local_metadata.clear();
        (&mut self.tx.local_metadata)
            .to_staged(merge_changes)
            .to_lazy()
            .promote();
        self.cleanup_local_metadata();

        Ok(update_as_of as i64)
    }

    pub fn get_updates(&self) -> CoreResult<GetUpdatesResponse> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let last_synced = self
            .tx
            .last_synced
            .get(&OneKey {})
            .copied()
            .unwrap_or_default() as u64;
        let remote_changes = self
            .client
            .request(account, GetUpdatesRequest { since_metadata_version: last_synced })?;
        Ok(remote_changes)
    }

    pub fn prune_remote_orphans(
        &mut self, remote_changes: Vec<SignedFile>,
    ) -> CoreResult<Vec<SignedFile>> {
        let me = Owner(self.get_public_key()?);
        let remote = (&self.tx.base_metadata).stage(remote_changes).to_lazy();
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

    fn prune(&mut self) -> CoreResult<()> {
        let account = self.get_account()?.clone();
        let mut local = (&self.tx.base_metadata)
            .stage(&self.tx.local_metadata)
            .to_lazy();
        let base_ids = local.tree.base.owned_ids();
        let server_ids = self.client.request(&account, GetFileIdsRequest {})?.ids;

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?.into_iter());
        }

        for id in &prunable_ids {
            if let Some(base_file) = local.tree.base.maybe_find(id) {
                document_repo::delete(self.config, id, base_file.document_hmac())?;
            }
            if let Some(local_file) = local.maybe_find(id) {
                document_repo::delete(self.config, id, local_file.document_hmac())?;
            }
        }

        let mut base_staged = (&mut self.tx.base_metadata).to_lazy().stage(None);
        base_staged.tree.removed = prunable_ids.clone();
        base_staged.promote();

        let mut local_staged = (&mut self.tx.local_metadata).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids;
        local_staged.promote();

        Ok(())
    }

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata<F>(&mut self, update_sync_progress: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncProgressOperation),
    {
        // remote = local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        let local = (&self.tx.base_metadata)
            .stage(&self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PushMetadata));
        for id in local.tree.staged.owned_ids() {
            let mut local_change = local.tree.staged.find(&id)?.timestamped_value.value.clone();
            let maybe_base_file = local.tree.base.maybe_find(&id);

            // change everything but document hmac and re-sign
            local_change.document_hmac =
                maybe_base_file.and_then(|f| f.timestamped_value.value.document_hmac);
            let local_change = local_change.sign(account)?;

            local_changes_no_digests.push(local_change.clone());
            let file_diff = FileDiff { old: maybe_base_file.cloned(), new: local_change };
            updates.push(file_diff);
        }
        println!("updates: {:?}", updates);
        if !updates.is_empty() {
            self.client.request(account, UpsertRequest { updates })?;
        }

        // base = local
        (&mut self.tx.base_metadata)
            .to_lazy()
            .stage(local_changes_no_digests)
            .promote();
        self.cleanup_local_metadata();

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, update_sync_progress: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncProgressOperation),
    {
        let mut local = (&self.tx.base_metadata)
            .stage(&self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut local_changes_digests_only = Vec::new();
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

            let local_change = local_change.sign(account)?;
            let local_document_change =
                document_repo::get(self.config, &id, local_change.document_hmac())?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local.name(&id, account)?),
            ));

            // base = local (document)
            document_repo::insert(
                self.config,
                &id,
                local_change.document_hmac(),
                &local_document_change,
            )?;

            // remote = local
            self.client.request(
                account,
                ChangeDocRequest {
                    diff: FileDiff { old: Some(base_file), new: local_change.clone() },
                    new_content: local_document_change,
                },
            )?;

            local_changes_digests_only.push(local_change);
        }

        // base = local (metadata)
        (&mut self.tx.base_metadata)
            .to_lazy()
            .stage(local_changes_digests_only)
            .promote();
        self.cleanup_local_metadata();

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self) -> CoreResult<WorkCalculated> {
        let updates = self.get_updates()?;
        let most_recent_update_from_server = updates.as_of_metadata_version;
        let mut remote_changes = updates.file_metadata;

        // prune prunable files
        remote_changes = self.prune_remote_orphans(remote_changes)?;

        // calculate work
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let mut work_units: Vec<WorkUnit> = Vec::new();
        {
            let mut remote = (&self.tx.base_metadata).stage(remote_changes).to_lazy();
            for id in remote.tree.staged.owned_ids() {
                if remote.tree.staged.maybe_find(&id) != remote.tree.base.maybe_find(&id) {
                    work_units.push(WorkUnit::ServerChange {
                        metadata: remote.finalize(
                            &id,
                            account,
                            &mut self.tx.username_by_public_key,
                        )?,
                    });
                }
            }
        }
        {
            let mut local = (&self.tx.base_metadata)
                .stage(&self.tx.local_metadata)
                .to_lazy();
            for id in local.tree.staged.owned_ids() {
                work_units.push(WorkUnit::LocalChange {
                    metadata: local.finalize(&id, account, &mut self.tx.username_by_public_key)?,
                });
            }
        }

        Ok(WorkCalculated { work_units, most_recent_update_from_server })
    }

    fn populate_public_key_cache(&mut self) -> CoreResult<()> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut all_owners = HashSet::new();
        for file in self.tx.base_metadata.get_all().values() {
            for user_access_key in file.user_access_keys() {
                all_owners.insert(Owner(user_access_key.encrypted_by));
                all_owners.insert(Owner(user_access_key.encrypted_for));
            }
        }
        for file in self.tx.local_metadata.get_all().values() {
            for user_access_key in file.user_access_keys() {
                all_owners.insert(Owner(user_access_key.encrypted_by));
                all_owners.insert(Owner(user_access_key.encrypted_for));
            }
        }

        for owner in all_owners {
            if !self.tx.username_by_public_key.exists(&owner) {
                let username = self
                    .client
                    .request(account, GetUsernameRequest { key: owner.0 })?
                    .username;
                self.tx
                    .username_by_public_key
                    .insert(owner, username.clone());
                self.tx.public_key_by_username.insert(username, owner);
            }
        }

        Ok(())
    }

    // todo: check only necessary ids
    fn cleanup_local_metadata(&mut self) {
        (&self.tx.base_metadata)
            .stage(&mut self.tx.local_metadata)
            .prune();
    }
}
