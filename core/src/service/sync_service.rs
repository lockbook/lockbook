use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::api_service;
use crate::CoreResult;
use crate::{Config, CoreError, RequestContext};
use lockbook_shared::account::Account;
use lockbook_shared::api::{
    ChangeDocRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
    GetUpdatesResponse,
};
use lockbook_shared::clock;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, FileDiff};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;
use std::collections::HashMap;

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
        &mut self, config: &Config, maybe_update_sync_progress: Option<F>,
    ) -> Result<(), CoreError> {
        // initialize sync progress: 3 metadata pulls + 1 metadata push + num local doc changes
        // note: num doc changes can change as a result of pull (can make new/changes docs deleted or add new docs from merge conflicts)
        let mut num_doc_changes = 0;
        for (id, local_change) in self.tx.local_metadata.get_all() {
            if let Some(base_file) = self.tx.base_metadata.get(&id) {
                if local_change.document_hmac() != base_file.document_hmac() {
                    num_doc_changes += 1;
                }
            } else {
                if local_change.document_hmac().is_some() {
                    num_doc_changes += 1;
                }
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

        self.pull(config, &mut update_sync_progress)?;
        self.push_metadata(&mut update_sync_progress)?;
        self.push_documents(&mut update_sync_progress)?;
        self.tx.last_synced.insert(OneKey {}, clock::get_time().0);
        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, config: &Config, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        // fetch metadata updates
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PullMetadata));
        let mut remote_changes = self.get_updates(account)?.file_metadata;

        // initialize root if this is the first pull on this device
        if self.tx.root.get(&OneKey {}).is_none() {
            let root = remote_changes
                .all_files()?
                .into_iter()
                .filter(|f| f.is_root())
                .next()
                .ok_or(CoreError::RootNonexistent)?;
            self.tx.root.insert(OneKey {}, *root.id());
        }

        // prune prunable files
        remote_changes =
            Self::prune(&mut self.tx.base_metadata, remote_changes, &mut self.tx.local_metadata)?;

        // track work
        {
            let base = self.tx.base_metadata.to_lazy();
            let mut num_documents_to_pull = 0;
            for id in remote_changes.owned_ids() {
                let maybe_base_hmac = base.maybe_find(&id).map(|f| f.document_hmac()).flatten();
                let maybe_remote_hmac = remote_changes.find(&id)?.document_hmac();
                if should_pull_document(maybe_base_hmac, maybe_remote_hmac) {
                    num_documents_to_pull += 1;
                }
            }
            update_sync_progress(SyncProgressOperation::IncrementTotalWork(num_documents_to_pull));
        }

        // fetch document updates and local documents for merge (todo: don't hold these all in memory at the same time)
        let mut base_documents = HashMap::new();
        let mut remote_document_changes = HashMap::new();
        let mut local_document_changes = HashMap::new();
        remote_changes = {
            let mut remote = self.tx.base_metadata.stage(remote_changes).to_lazy();
            for id in remote.tree.staged.owned_ids() {
                if let Some(&hmac) = remote.find(&id)?.document_hmac() {
                    update_sync_progress(SyncProgressOperation::StartWorkUnit(
                        ClientWorkUnit::PullDocument(remote.name(&id, account)?),
                    ));
                    let remote_document_change = api_service::request(
                        account,
                        GetDocumentRequest { id, hmac: hmac.clone() },
                    )?
                    .content;
                    document_repo::maybe_get(config, RepoSource::Base, &id)?
                        .map(|d| base_documents.insert(id, d));
                    remote_document_changes.insert(id, remote_document_change);
                    document_repo::maybe_get(config, RepoSource::Local, &id)?
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
                &remote_document_changes,
                &local_document_changes,
            )?;
            let (remote, _) = local.unstage();
            let (_, remote_changes) = remote.unstage();
            (remote_changes, merge_document_changes)
        };
        self.tx
            .base_metadata
            .stage(remote_changes)
            .to_lazy()
            .promote();
        for (id, document) in remote_document_changes {
            document_repo::insert(self.config, RepoSource::Base, id, &document)?;
        }
        for (id, document) in merge_document_changes {
            document_repo::insert(self.config, RepoSource::Local, id, &document)?;
        }

        Ok(())
    }

    fn prune<Base, Local>(
        base: Base, remote_changes: Vec<SignedFile>, local_changes: Local,
    ) -> CoreResult<Vec<SignedFile>>
    where
        Base: Stagable<F = SignedFile>,
        Local: Stagable<F = Base::F>,
    {
        let mut local = base.stage(remote_changes).stage(local_changes).to_lazy();
        for id in local.prunable_ids()? {
            local.remove(id);
        }
        let (remote, _) = local.unstage();
        let (_, remote_changes) = remote.unstage();
        Ok(remote_changes)
    }

    pub fn get_updates(&self, account: &Account) -> CoreResult<GetUpdatesResponse> {
        let last_synced = self
            .tx
            .last_synced
            .get(&OneKey {})
            .map(|&i| i)
            .unwrap_or_default() as u64;
        let remote_changes = api_service::request(
            &account,
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
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PushMetadata));

        // remote = local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        for id in (&mut self.tx.local_metadata).owned_ids() {
            let mut local_change = self
                .tx
                .local_metadata
                .get(&id)
                .ok_or(CoreError::FileNonexistent)?
                .timestamped_value
                .value
                .clone();
            let maybe_base_file = self.tx.base_metadata.get(&id);

            // change everything but document hmac and re-sign
            local_change.document_hmac = maybe_base_file
                .map(|f| f.timestamped_value.value.document_hmac)
                .flatten();
            let local_change = local_change.sign(account)?;

            local_changes_no_digests.push(local_change.clone());
            updates.push(FileDiff { old: maybe_base_file.cloned(), new: local_change });
        }
        if !updates.is_empty() {
            api_service::request(account, FileMetadataUpsertsRequest { updates })
                .map_err(CoreError::from)?;
        }

        // base = local
        self.tx
            .base_metadata
            .stage(local_changes_no_digests)
            .to_lazy()
            .promote();

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        // remote = local
        let mut local_changes_digests_only = Vec::new();
        let mut local = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        for id in local.tree.staged.owned_ids() {
            let base_file = local.tree.base.find(&id)?.clone();

            // change only document hmac and re-sign
            let mut local_change = base_file.timestamped_value.value.clone();
            local_change.document_hmac = local.find(&id)?.timestamped_value.value.document_hmac;

            let local_change = local_change.sign(account)?;
            let local_document_change = document_repo::get(self.config, RepoSource::Local, id)?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local.name(&id, account)?),
            ));
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

        // base = local
        self.tx
            .base_metadata
            .stage(local_changes_digests_only)
            .to_lazy()
            .promote();

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self) -> CoreResult<WorkCalculated> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        // fetch metadata updates
        let updates = self.get_updates(account)?;
        let mut remote_changes = updates.file_metadata;

        // prune prunable files
        remote_changes =
            Self::prune(&mut self.tx.base_metadata, remote_changes, &mut self.tx.local_metadata)?;

        // calculate work
        let mut work_units: Vec<WorkUnit> = vec![];
        {
            let mut remote = self.tx.base_metadata.stage(remote_changes).to_lazy();
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

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server: updates.as_of_metadata_version,
        })
    }
}
