use std::collections::{HashMap, HashSet};

use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::api_service;
use crate::CoreResult;
use crate::{CoreError, RequestContext};
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetUpdatesRequest, GetUpdatesResponse, UpsertRequest,
};
use lockbook_shared::core_tree::CoreTree;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, FileDiff, Owner};
use lockbook_shared::lazy::{LazyStage2, LazyStaged1, LazyTree};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;

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
    ) -> Result<(), CoreError> {
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

        self.validate()?;
        self.pull(&mut update_sync_progress)?;
        self.push_metadata(&mut update_sync_progress)?;
        self.push_documents(&mut update_sync_progress)?;
        self.pull(&mut update_sync_progress)?;
        self.prune()?;
        self.validate()?;

        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, update_sync_progress: &mut F) -> CoreResult<()>
    where
        F: FnMut(SyncProgressOperation),
    {
        // fetch metadata updates
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PullMetadata));
        let updates = self.get_updates()?;
        let remote_changes = updates.file_metadata;
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
        for owner in self.owners(&remote_changes)? {
            let base = CoreTree { owner, metas: &mut self.tx.base_metadata };
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

        let remote_changes_by_owner = self.partition_files(remote_changes)?;
        for (owner, remote_changes) in remote_changes_by_owner {
            self.pull_owner(owner, remote_changes, update_sync_progress)?;
        }

        self.tx.last_synced.insert(OneKey {}, update_as_of as i64);

        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull_owner<F>(
        &mut self, owner: Owner, mut remote_changes: Vec<SignedFile>, update_sync_progress: &mut F,
    ) -> CoreResult<()>
    where
        F: FnMut(SyncProgressOperation),
    {
        // prune prunable files
        remote_changes = self.prune_remote_orphans(owner, remote_changes)?;
        self.validate()?;

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
            let mut remote =
                LazyTree::base_tree(owner, &mut self.tx.base_metadata).stage(remote_changes);
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
            let local = LazyStage2::core_tree_with_remote(
                owner,
                &mut self.tx.base_metadata,
                remote_changes,
                &mut self.tx.local_metadata,
            );
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
        LazyTree::base_tree(owner, &mut self.tx.base_metadata)
            .stage(remote_changes)
            .promote();
        for (id, document) in remote_document_changes {
            document_repo::insert(self.config, RepoSource::Base, id, &document)?;
        }
        for (id, document) in merge_document_changes {
            document_repo::insert(self.config, RepoSource::Local, id, &document)?;
        }

        self.validate()?;
        Ok(())
    }

    // todo: remove
    pub fn validate(&mut self) -> CoreResult<()> {
        // todo: all owners
        for owner in [&Owner(self.get_public_key()?)] {
            let mut base = LazyTree::base_tree(*owner, &mut self.tx.base_metadata);
            let local_changes = LazyTree::base_tree(*owner, &mut self.tx.local_metadata).tree;
            base.validate()?;
            let mut local = base.stage(local_changes);
            local.validate()?;
        }
        Ok(())
    }

    // todo: cache or something
    pub fn owners(&self, remote_changes: &[SignedFile]) -> CoreResult<HashSet<Owner>> {
        let mut result = HashSet::new();
        for file in self.tx.base_metadata.get_all().values() {
            result.insert(file.owner());
        }
        for file in remote_changes {
            result.insert(file.owner());
        }
        for file in self.tx.local_metadata.get_all().values() {
            result.insert(file.owner());
        }
        Ok(result)
    }

    pub fn prune(&mut self) -> CoreResult<()> {
        for owner in self.owners(&Vec::new())? {
            self.prune_owner(owner)?;
        }

        Ok(())
    }

    fn prune_owner(&mut self, owner: Owner) -> CoreResult<()> {
        let local =
            LazyStaged1::core_tree(owner, &mut self.tx.base_metadata, &mut self.tx.local_metadata);
        let (mut local, prunable_ids) = local.prunable_ids()?;
        for id in prunable_ids {
            local.remove(id);
        }
        Ok(())
    }

    pub fn prune_remote_orphans(
        &mut self, owner: Owner, remote_changes: Vec<SignedFile>,
    ) -> CoreResult<Vec<SignedFile>> {
        let me = Owner(self.get_public_key()?);
        let remote = LazyTree::base_tree(owner, &mut self.tx.base_metadata).stage(remote_changes);
        let mut result = Vec::new();
        for id in remote.tree.staged.owned_ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some() || meta.shared_access(&me) {
                result.push(remote.find(&id)?.clone()); // todo: don't clone
            }
        }
        Ok(result)
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

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata<F>(&mut self, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        for owner in self.owners(&Vec::new())? {
            self.push_metadata_owner(owner, update_sync_progress)?;
        }

        Ok(())
    }

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata_owner<F>(
        &mut self, owner: Owner, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        // remote = local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        let local =
            LazyStaged1::core_tree(owner, &mut self.tx.base_metadata, &mut self.tx.local_metadata);
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
        LazyTree::base_tree(owner, &mut self.tx.base_metadata)
            .stage(local_changes_no_digests)
            .promote();

        self.validate()?;
        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        for owner in self.owners(&Vec::new())? {
            self.push_documents_owner(owner, update_sync_progress)?;
        }

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents_owner<F>(
        &mut self, owner: Owner, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let mut local =
            LazyStaged1::core_tree(owner, &mut self.tx.base_metadata, &mut self.tx.local_metadata);
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
            let local_document_change = document_repo::get(self.config, RepoSource::Local, id)?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local.name(&id, account)?),
            ));

            // base = local (document)
            document_repo::insert(self.config, RepoSource::Base, id, &local_document_change)?;

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
        LazyTree::base_tree(owner, &mut self.tx.base_metadata)
            .stage(local_changes_digests_only)
            .promote();

        self.validate()?;
        Ok(())
    }

    fn partition_files(
        &self, files: Vec<SignedFile>,
    ) -> CoreResult<HashMap<Owner, Vec<SignedFile>>> {
        let mut result = HashMap::new();
        for owner in self.owners(&files)? {
            result.insert(owner, Vec::new());
        }
        for file in files {
            if let Some(v) = result.get_mut(&file.owner()) {
                v.push(file);
            }
        }
        Ok(result)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self) -> CoreResult<WorkCalculated> {
        let updates = self.get_updates()?;
        let mut result = WorkCalculated {
            work_units: Vec::new(),
            most_recent_update_from_server: updates.as_of_metadata_version,
        };
        let remote_changes_by_owner = self.partition_files(updates.file_metadata)?;
        for (owner, remote_changes) in remote_changes_by_owner {
            result
                .work_units
                .extend(self.calculate_work_owner(owner, remote_changes)?);
        }
        Ok(result)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    fn calculate_work_owner(
        &mut self, owner: Owner, mut remote_changes: Vec<SignedFile>,
    ) -> CoreResult<Vec<WorkUnit>> {
        // prune prunable files
        remote_changes = self.prune_remote_orphans(owner, remote_changes)?;

        // calculate work
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let mut work_units: Vec<WorkUnit> = Vec::new();
        {
            let mut remote =
                LazyTree::base_tree(owner, &mut self.tx.base_metadata).stage(remote_changes);
            for id in remote.tree.staged.owned_ids() {
                work_units
                    .push(WorkUnit::ServerChange { metadata: remote.finalize(&id, account)? });
            }
        }
        {
            let mut local = LazyStaged1::core_tree(
                owner,
                &mut self.tx.base_metadata,
                &mut self.tx.local_metadata,
            );
            for id in local.tree.staged.owned_ids() {
                work_units.push(WorkUnit::LocalChange { metadata: local.finalize(&id, account)? });
            }
        }

        Ok(work_units)
    }
}
