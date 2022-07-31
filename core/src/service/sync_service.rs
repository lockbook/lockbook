use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::{api_service, file_service};
use crate::CoreResult;
use crate::{Config, CoreError, RequestContext};
use lockbook_shared::account::Account;
use lockbook_shared::api::{
    ChangeDocRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
    GetUpdatesResponse,
};
use lockbook_shared::clock;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, FileDiff};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;
use std::collections::HashMap;
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
        self.push_metadata(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.push_documents(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.tx.last_synced.insert(OneKey {}, clock::get_time().0);
        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, config: &Config, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        todo!()
    }

    fn get_updates(&mut self, account: &Account) -> CoreResult<GetUpdatesResponse> {
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
    fn push_metadata<F>(
        &mut self, _config: &Config, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = &self.get_account()?;
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PushMetadata));

        // update remote to local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        for id in (&mut self.tx.local_metadata).owned_ids() {
            let mut local_change = self
                .tx
                .local_metadata
                .get(&id)
                .ok_or(CoreError::FileNonexistent)?
                .timestamped_value
                .value;
            let maybe_base_file = self.tx.base_metadata.get(&id);

            // reset document hmac and re-sign; documents and their hmacs are pushed separately
            local_change.document_hmac = maybe_base_file
                .map(|f| f.timestamped_value.value.document_hmac)
                .flatten();
            let local_change = local_change.sign(account)?;

            local_changes_no_digests.push(local_change);
            updates.push(FileDiff { old: maybe_base_file.cloned(), new: local_change });
        }
        if !updates.is_empty() {
            api_service::request(account, FileMetadataUpsertsRequest { updates })
                .map_err(CoreError::from)?;
        }

        // update base to local
        self.tx
            .base_metadata
            .stage(local_changes_no_digests)
            .to_lazy()
            .promote();

        Ok(())
    }

    /// Updates remote and base files to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(
        &mut self, config: &Config, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = &self.get_account()?;

        let mut local_changes_digests_only = Vec::new();
        let local = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        for id in local.tree.staged.owned_ids() {
            let base_file = self
                .tx
                .local_metadata
                .get(&id)
                .ok_or(CoreError::FileNonexistent)?;

            // change only document hmac and re-sign; other fields pushed separately
            let mut local_change = base_file.timestamped_value.value.clone();
            local_change.document_hmac = self
                .tx
                .base_metadata
                .get(&id)
                .map(|f| f.timestamped_value.value.document_hmac)
                .flatten();

            let local_change = local_change.sign(account)?;
            let local_document_change = document_repo::get(config, RepoSource::Local, id)?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local.name(&id, account)?),
            ));
            api_service::request(
                account,
                ChangeDocRequest {
                    diff: FileDiff { old: Some(base_file.clone()), new: local_change },
                    new_content: local_document_change,
                },
            )
            .map_err(CoreError::from)?;

            local_changes_digests_only.push(local_change);
        }

        // update base to local
        self.tx
            .base_metadata
            .stage(local_changes_digests_only)
            .to_lazy()
            .promote();

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self, config: &Config) -> CoreResult<WorkCalculated> {
        todo!()
    }
}

fn get_document<Base, Remote>(
    account: &Account, base: Base, remote_changes: Remote, id: Uuid,
) -> CoreResult<Option<EncryptedDocument>>
where
    Base: Stagable<F = SignedFile>,
    Remote: Stagable<F = SignedFile>,
{
    let remote = base.stage(remote_changes).to_lazy();
    let maybe_hmac = remote.find(&id)?.document_hmac();
    Ok(if let Some(hmac) = maybe_hmac {
        Some(api_service::request(account, GetDocumentRequest { id, hmac: hmac.clone() })?.content)
    } else {
        None
    })
}
