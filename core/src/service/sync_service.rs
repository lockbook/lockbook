use crate::OneKey;
use crate::{CoreError, RequestContext};
use crate::{CoreResult, Requester};
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetFileIdsRequest, GetUpdatesRequest, GetUpdatesResponse,
    GetUsernameRequest, UpsertRequest,
};
use lockbook_shared::document_repo;
use lockbook_shared::file::like::FileLike;
use lockbook_shared::file::metadata::{FileDiff, Owner};
use lockbook_shared::file::signed::SignedFile;
use lockbook_shared::file::File;
use lockbook_shared::tree::lazy::LazyTreeLike;
use lockbook_shared::tree::like::{TreeLike, TreeLikeMut};
use lockbook_shared::tree::stagable::StagableMut;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

#[derive(Clone)]
pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

enum SyncOperation {
    PullMetadataStart,
    PullMetadataEnd(Vec<File>),
    PushMetadataStart(Vec<File>),
    PushMetadataEnd,
    PullDocumentStart(File),
    PullDocumentEnd,
    PushDocumentStart(File),
    PushDocumentEnd,
}

fn get_work_units(op: &SyncOperation) -> Vec<WorkUnit> {
    let mut work_units: Vec<WorkUnit> = Vec::new();
    match op {
        SyncOperation::PullMetadataEnd(files) => {
            for file in files {
                work_units.push(WorkUnit::ServerChange { metadata: file.clone() });
            }
        }
        SyncOperation::PushMetadataStart(files) => {
            for file in files {
                work_units.push(WorkUnit::LocalChange { metadata: file.clone() });
            }
        }
        SyncOperation::PullMetadataStart
        | SyncOperation::PushMetadataEnd
        | SyncOperation::PullDocumentStart(_)
        | SyncOperation::PullDocumentEnd
        | SyncOperation::PushDocumentStart(_)
        | SyncOperation::PushDocumentEnd => {}
    }
    work_units
}

impl<Client: Requester> RequestContext<'_, '_, Client> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self) -> CoreResult<WorkCalculated> {
        let mut work_units: Vec<WorkUnit> = Vec::new();
        let update_as_of = self
            .sync_helper(true, &mut |op: SyncOperation| work_units.extend(get_work_units(&op)))?;

        Ok(WorkCalculated { work_units, most_recent_update_from_server: update_as_of as u64 })
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync<F: Fn(SyncProgress)>(
        &mut self, maybe_update_sync_progress: Option<F>,
    ) -> CoreResult<WorkCalculated> {
        let mut work_units: Vec<WorkUnit> = Vec::new();
        let update_as_of = if let Some(update_sync_progress) = maybe_update_sync_progress {
            let mut sync_progress = SyncProgress {
                total: 0,
                progress: 0,
                current_work_unit: ClientWorkUnit::PullMetadata,
            };
            self.sync_helper(true, &mut |op: SyncOperation| {
                sync_progress.total += get_work_units(&op).len()
            })?;
            self.sync_helper(false, &mut |op: SyncOperation| {
                work_units.extend(get_work_units(&op));
                match op {
                    SyncOperation::PullMetadataStart => {
                        sync_progress.current_work_unit = ClientWorkUnit::PullMetadata;
                    }
                    SyncOperation::PushMetadataStart(_) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PushMetadata;
                    }
                    SyncOperation::PullDocumentStart(file) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PullDocument(file.name);
                    }
                    SyncOperation::PushDocumentStart(file) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PushDocument(file.name);
                    }
                    SyncOperation::PullMetadataEnd(_)
                    | SyncOperation::PushMetadataEnd
                    | SyncOperation::PullDocumentEnd
                    | SyncOperation::PushDocumentEnd => {
                        sync_progress.progress += 1;
                    }
                }
                update_sync_progress(sync_progress.clone());
            })?
        } else {
            self.sync_helper(false, &mut |op: SyncOperation| {
                work_units.extend(get_work_units(&op))
            })?
        };
        Ok(WorkCalculated { work_units, most_recent_update_from_server: update_as_of as u64 })
    }

    fn sync_helper<F>(&mut self, dry_run: bool, report_sync_operation: &mut F) -> CoreResult<i64>
    where
        F: FnMut(SyncOperation),
    {
        let update_as_of = self.pull(dry_run, report_sync_operation)?;
        if !dry_run {
            self.tx.last_synced.insert(OneKey {}, update_as_of);
        }
        self.push_metadata(dry_run, report_sync_operation)?;
        if !dry_run {
            self.prune()?;
        }
        self.push_documents(dry_run, report_sync_operation)?;
        let update_as_of = self.pull(dry_run, report_sync_operation)?;
        if !dry_run {
            self.tx.last_synced.insert(OneKey {}, update_as_of);
        }
        Ok(update_as_of)
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge> unless dry_run == true
    fn pull<F>(&mut self, dry_run: bool, report_sync_operation: &mut F) -> CoreResult<i64>
    where
        F: FnMut(SyncOperation),
    {
        // fetch metadata updates
        report_sync_operation(SyncOperation::PullMetadataStart);
        let updates = self.get_updates()?;
        let mut remote_changes = updates.file_metadata;
        let update_as_of = updates.as_of_metadata_version;

        // pre-process changes
        remote_changes = self.prune_remote_orphans(remote_changes)?;

        // populate key cache
        self.populate_public_key_cache(&remote_changes)?;

        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        // track work
        {
            let mut remote = (&mut self.tx.base_metadata)
                .stage_mut(&mut remote_changes)
                .to_lazy();

            let finalized_remote_changes = remote.resolve_and_finalize(
                account,
                remote.tree.staged.owned_ids().into_iter(),
                &mut self.tx.username_by_public_key,
            )?;
            report_sync_operation(SyncOperation::PullMetadataEnd(finalized_remote_changes));

            let (_, remote_changes) = remote.unstage();
            remote_changes
        };

        // initialize root if this is the first pull on this device
        if self.tx.root.get(&OneKey {}).is_none() {
            let root = remote_changes
                .all_files()?
                .into_iter()
                .find(|f| f.is_root())
                .ok_or(CoreError::RootNonexistent)?;
            self.tx.root.insert(OneKey {}, *root.id());
        }

        // fetch document updates and local documents for merge
        let mut remote_document_changes = HashSet::new();
        {
            let mut remote = (&mut self.tx.base_metadata)
                .to_lazy()
                .stage(&mut remote_changes);
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
                    report_sync_operation(SyncOperation::PullDocumentStart(remote.finalize(
                        &id,
                        account,
                        &mut self.tx.username_by_public_key,
                    )?));
                    if !dry_run {
                        let remote_document = self
                            .client
                            .request(account, GetDocRequest { id, hmac: remote_hmac })?
                            .content;
                        document_repo::insert(
                            self.config,
                            &id,
                            Some(&remote_hmac),
                            &remote_document,
                        )?;
                    }
                    report_sync_operation(SyncOperation::PullDocumentEnd);
                    remote_document_changes.insert(id);
                }
            }
            let (_, remote_changes) = remote.unstage();
            remote_changes
        };

        // base = remote; local = merge
        let mut merge_changes = Vec::new();
        {
            let local = (&mut self.tx.base_metadata)
                .stage_mut(&mut remote_changes)
                .stage_mut(&mut self.tx.local_metadata)
                .stage_mut(&mut merge_changes)
                .to_lazy();

            let merge = local.merge(self.config, dry_run, account, &remote_document_changes)?;
            let (local, merge_changes) = merge.unstage();
            let (remote, _) = local.unstage();
            let (_, remote_changes) = remote.unstage();
            (remote_changes, merge_changes)
        };

        if !dry_run {
            (&mut self.tx.base_metadata)
                .stage_mut(&mut remote_changes)
                .to_lazy()
                .promote();
            (&mut self.tx.local_metadata)
                .stage_mut(&mut merge_changes)
                .to_lazy()
                .promote();
            self.reset_deleted_files()?;
        }

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
        &mut self, mut remote_changes: Vec<SignedFile>,
    ) -> CoreResult<Vec<SignedFile>> {
        let me = Owner(self.get_public_key()?);
        let remote = (&mut self.tx.base_metadata)
            .to_lazy()
            .stage(&mut remote_changes);
        let mut result = Vec::new();
        for id in remote.tree.staged.owned_ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || meta
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == me.0)
            {
                result.push(remote.find(&id)?.clone());
            }
        }
        Ok(result)
    }

    fn reset_deleted_files(&mut self) -> CoreResult<()> {
        // resets all changes to files that are implicitly deleted, then explicitly deletes them
        // we don't want to push updates to deleted documents and we might as well not push updates to deleted metadata
        // we must explicitly delete a file which is moved into a deleted folder because otherwise resetting it makes it no longer deleted
        let account = self.get_account()?.clone();

        let mut tree = (&mut self.tx.base_metadata).to_lazy();
        let mut already_deleted = HashSet::new();
        for id in tree.owned_ids() {
            if tree.calculate_deleted(&id)? {
                already_deleted.insert(id);
            }
        }

        let mut tree = (&mut self.tx.base_metadata)
            .stage_mut(&mut self.tx.local_metadata)
            .to_lazy();
        let mut local_change_removals = HashSet::new();
        let mut local_change_resets = Vec::new();

        for id in tree.tree.staged.owned_ids() {
            if let Some(base_file) = tree.tree.base.maybe_find(&id) {
                let mut base_file = base_file.timestamped_value.value.clone();
                if already_deleted.contains(&id) {
                    // reset file
                    local_change_resets.push(base_file.sign(&account)?);
                } else if tree.calculate_deleted(&id)? {
                    // reset everything but set deleted=true
                    base_file.is_deleted = true;
                    local_change_resets.push(base_file.sign(&account)?);
                }
            } else if tree.calculate_deleted(&id)? {
                // delete
                local_change_removals.insert(id);
            }
        }

        for id in local_change_removals {
            tree.remove(id);
        }
        tree.stage(&mut local_change_resets).promote();

        Ok(())
    }

    fn prune(&mut self) -> CoreResult<()> {
        let account = self.get_account()?.clone();
        let mut local = (&mut self.tx.base_metadata)
            .stage_mut(&mut self.tx.local_metadata)
            .to_lazy();
        let base_ids = local.tree.base.owned_ids();
        let server_ids = self.client.request(&account, GetFileIdsRequest {})?.ids;

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?.into_iter());
        }

        for id in prunable_ids {
            local.remove(id);
            if let Some(base_file) = local.tree.base.maybe_find(&id) {
                document_repo::delete(self.config, &id, base_file.document_hmac())?;
            }
            if let Some(local_file) = local.maybe_find(&id) {
                document_repo::delete(self.config, &id, local_file.document_hmac())?;
            }
        }
        Ok(())
    }

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata<F>(&mut self, dry_run: bool, report_sync_operation: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncOperation),
    {
        // remote = local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        let mut local = (&mut self.tx.base_metadata)
            .stage_mut(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let finalized_local_changes = local.resolve_and_finalize(
            account,
            local.tree.staged.owned_ids().into_iter(),
            &mut self.tx.username_by_public_key,
        )?;
        report_sync_operation(SyncOperation::PushMetadataStart(finalized_local_changes));
        if !dry_run {
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
            if !updates.is_empty() {
                self.client.request(account, UpsertRequest { updates })?;
            }
        }
        report_sync_operation(SyncOperation::PushMetadataEnd);

        // base = local
        (&mut self.tx.base_metadata)
            .to_lazy()
            .stage(&mut local_changes_no_digests)
            .promote();

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, dry_run: bool, report_sync_operation: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncOperation),
    {
        let mut local = (&mut self.tx.base_metadata)
            .stage_mut(&mut self.tx.local_metadata)
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

            report_sync_operation(SyncOperation::PushDocumentStart(local.finalize(
                &id,
                account,
                &mut self.tx.username_by_public_key,
            )?));
            if !dry_run {
                let local_document_change =
                    document_repo::get(self.config, &id, local_change.document_hmac())?;

                // base = local (document)
                // todo: is this required?
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
            }
            report_sync_operation(SyncOperation::PushDocumentEnd);

            local_changes_digests_only.push(local_change);
        }

        // base = local (metadata)
        if !dry_run {
            (&mut self.tx.base_metadata)
                .to_lazy()
                .stage(&mut local_changes_digests_only)
                .promote();
        }

        Ok(())
    }

    fn populate_public_key_cache(&mut self, files: &[SignedFile]) -> CoreResult<()> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut all_owners = HashSet::new();
        for file in files {
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
}
