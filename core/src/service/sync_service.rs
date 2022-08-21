use crate::repo::schema::OneKey;
use crate::service::api_service;
use crate::CoreResult;
use crate::{CoreError, RequestContext};
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetUpdatesRequest, GetUpdatesResponse, UpsertRequest,
};
use lockbook_shared::document_repo::{self, RepoSource};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, FileDiff, Owner};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

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

impl RequestContext<'_, '_> {
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
        self.push_documents(&mut update_sync_progress)?;
        let update_as_of = self.pull(&mut update_sync_progress)?;

        self.tx.last_synced.insert(OneKey {}, update_as_of);
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
            let base = self
                .tx
                .base_metadata
                .stage(&mut self.tx.local_metadata)
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

        // fetch document updates and local documents for merge (todo: don't hold these all in memory at the same time)
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let mut base_documents = HashMap::new();
        let mut remote_document_changes = HashMap::new();
        let mut local_document_changes = HashMap::new();
        remote_changes = {
            let mut remote = self.tx.base_metadata.to_lazy().stage(remote_changes);
            for id in remote.tree.staged.owned_ids() {
                if remote.calculate_deleted(&id)? {
                    continue;
                }
                if let Some(&remote_hmac) = remote.find(&id)?.document_hmac() {
                    let base_hmac = {
                        if let Some(base_file) = remote.tree.base.maybe_find(&id) {
                            base_file.document_hmac()
                        } else {
                            None
                        }
                    };
                    if base_hmac == Some(&remote_hmac) {
                        continue;
                    }
                    update_sync_progress(SyncProgressOperation::StartWorkUnit(
                        ClientWorkUnit::PullDocument(remote.name(&id, account)?),
                    ));
                    let remote_document_change =
                        api_service::request(account, GetDocRequest { id, hmac: remote_hmac })?
                            .content;
                    document_repo::maybe_get(self.config, RepoSource::Base, &id)?
                        .map(|d| base_documents.insert(id, d));
                    remote_document_changes.insert(id, remote_document_change);
                    document_repo::maybe_get(self.config, RepoSource::Local, &id)?
                        .map(|d| local_document_changes.insert(id, d));
                }
            }
            let (_, remote_changes) = remote.unstage();
            remote_changes
        };

        // base = remote; local = merge
        let (remote_changes, merge_document_changes) = {
            let local = self
                .tx
                .base_metadata
                .stage(remote_changes)
                .stage(&mut self.tx.local_metadata)
                .to_lazy();

            let (local, merge_document_changes) = local.merge(
                account,
                &base_documents,
                &local_document_changes,
                &remote_document_changes,
            )?;
            let (remote, _) = local.unstage();
            let (_, remote_changes) = remote.unstage();
            (remote_changes, merge_document_changes)
        };

        let tree = self
            .tx
            .base_metadata
            .stage(remote_changes)
            .to_lazy()
            .promote()
            .stage(&mut self.tx.local_metadata);
        for (id, document) in remote_document_changes {
            tree.write_document_content(self.config, RepoSource::Base, &id, &document)?;
        }
        for (id, document) in merge_document_changes {
            tree.write_document_content(self.config, RepoSource::Local, &id, &document)?;
        }
        self.reset_deleted_files()?;
        self.prune()?;

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
        let remote_changes = api_service::request(
            account,
            GetUpdatesRequest { since_metadata_version: last_synced },
        )?;
        Ok(remote_changes)
    }

    pub fn prune_remote_orphans(
        &mut self, remote_changes: Vec<SignedFile>,
    ) -> CoreResult<Vec<SignedFile>> {
        let me = Owner(self.get_public_key()?);
        let remote = self.tx.base_metadata.to_lazy().stage(remote_changes);
        let mut result = Vec::new();
        for id in remote.tree.staged.owned_ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some() || meta.access_mode(&me).is_some() {
                result.push(remote.find(&id)?.clone()); // todo: don't clone
            }
        }
        Ok(result)
    }

    fn reset_deleted_files(&mut self) -> CoreResult<()> {
        // resets all changes to files that are implicitly deleted, then explicitly deletes them
        // we don't want to push updates to deleted documents and we might as well not push updates to deleted metadata
        // we must explicitly delete a file which is moved into a deleted folder because otherwise resetting it makes it no longer deleted
        // todo: document repo deletions are not atomic and an interruption during sync can cause digests to become out of date
        let account = self.get_account()?.clone();

        let mut tree = self.tx.base_metadata.to_lazy();
        let mut already_deleted = HashSet::new();
        for id in tree.owned_ids() {
            if tree.calculate_deleted(&id)? {
                already_deleted.insert(id);
            }
        }

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let mut local_change_removals = HashSet::new();
        let mut local_change_resets = Vec::new();

        for id in tree.tree.staged.owned_ids() {
            let local_file = tree.find(&id)?.timestamped_value.value.clone();
            if let Some(base_file) = tree.tree.base.maybe_find(&id) {
                let mut base_file = base_file.timestamped_value.value.clone();
                if already_deleted.contains(&id) {
                    // reset file
                    if base_file.document_hmac != local_file.document_hmac
                        && local_file.document_hmac.is_some()
                    {
                        document_repo::delete(self.config, RepoSource::Local, &id)?;
                    }
                    local_change_resets.push(base_file.sign(&account)?);
                } else if tree.calculate_deleted(&id)? {
                    // reset everything but set deleted=true
                    base_file.is_deleted = true;
                    if base_file.document_hmac != local_file.document_hmac
                        && local_file.document_hmac.is_some()
                    {
                        document_repo::delete(self.config, RepoSource::Local, &id)?;
                    }
                    local_change_resets.push(base_file.sign(&account)?);
                }
            } else if tree.calculate_deleted(&id)? {
                // delete
                if local_file.document_hmac.is_some() {
                    document_repo::delete(self.config, RepoSource::Local, &id)?;
                }
                local_change_removals.insert(id);
            }
        }

        for id in local_change_removals {
            tree.remove(id);
        }
        tree.stage(local_change_resets).promote();

        Ok(())
    }

    fn prune(&mut self) -> CoreResult<()> {
        let local = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let (mut local, prunable_ids) = local.prunable_ids()?;
        for id in prunable_ids {
            local.remove(id);
        }
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
        let local = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
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
        if !updates.is_empty() {
            api_service::request(account, UpsertRequest { updates })?;
        }

        // base = local
        self.tx
            .base_metadata
            .to_lazy()
            .stage(local_changes_no_digests)
            .promote();

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, update_sync_progress: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncProgressOperation),
    {
        let mut local = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
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
            let local_document_change = document_repo::get(self.config, RepoSource::Local, &id)?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local.name(&id, account)?),
            ));

            // base = local (document)
            document_repo::insert(self.config, RepoSource::Base, &id, &local_document_change)?;
            document_repo::delete(self.config, RepoSource::Local, &id)?;

            // remote = local
            api_service::request(
                account,
                ChangeDocRequest {
                    diff: FileDiff { old: Some(base_file), new: local_change.clone() },
                    new_content: local_document_change,
                },
            )
            .map_err(CoreError::from)?;

            local_changes_digests_only.push(local_change);
        }

        // base = local (metadata)
        self.tx
            .base_metadata
            .to_lazy()
            .stage(local_changes_digests_only)
            .promote();

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
            let mut remote = self.tx.base_metadata.to_lazy().stage(remote_changes);
            for id in remote.tree.staged.owned_ids() {
                work_units
                    .push(WorkUnit::ServerChange { metadata: remote.finalize(&id, account)? });
            }
        }
        {
            let mut local = self
                .tx
                .base_metadata
                .stage(&mut self.tx.local_metadata)
                .to_lazy();
            for id in local.tree.staged.owned_ids() {
                work_units.push(WorkUnit::LocalChange { metadata: local.finalize(&id, account)? });
            }
        }

        Ok(WorkCalculated { work_units, most_recent_update_from_server })
    }
}
