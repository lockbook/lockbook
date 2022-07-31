use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::{api_service, file_service};
use crate::CoreResult;
use crate::{Config, CoreError, RequestContext};
use lockbook_shared::account::Account;
use lockbook_shared::api::{
    FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest, GetUpdatesResponse,
};
use lockbook_shared::clock;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::DocumentHmac;
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
        // fetch metadata updates
        let account = self.get_account()?;
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PullMetadata));
        // todo: if this doesn't need to be mut, prune is broken
        let remote_changes = self.get_updates(account)?.file_metadata;

        // prune prunable files
        {
            let mut local = self
                .tx
                .base_metadata
                .stage(remote_changes)
                .stage(&mut self.tx.local_metadata)
                .to_lazy();
            for id in local.prunable_ids()? {
                local.remove(id);
            }
        }

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

        // fetch document updates and local documents for merge
        let mut base_documents = HashMap::new();
        let mut remote_document_changes = HashMap::new();
        let mut local_document_changes = HashMap::new();
        {
            let mut remote = self.tx.base_metadata.stage(remote_changes).to_lazy();
            for id in remote_changes.owned_ids() {
                if let Some(remote_document_change) =
                    get_document(account, &mut self.tx.base_metadata, remote_changes, id)?
                {
                    update_sync_progress(SyncProgressOperation::StartWorkUnit(
                        ClientWorkUnit::PullDocument(remote.name(&id, account)?),
                    ));
                    document_repo::maybe_get(config, RepoSource::Base, &id)?
                        .map(|d| base_documents.insert(id, d));
                    remote_document_changes.insert(id, remote_document_change);
                    document_repo::maybe_get(config, RepoSource::Local, &id)?
                        .map(|d| local_document_changes.insert(id, d));
                }
            }
        };

        // merge and promote
        let (local, merge_document_changes) = {
            let mut local = self
                .tx
                .base_metadata
                .stage(remote_changes)
                .stage(&mut self.tx.local_metadata)
                .to_lazy();
            local.merge(
                account,
                &base_documents,
                &remote_document_changes,
                &local_document_changes,
            )?
        };
        self.tx
            .base_metadata
            .stage(remote_changes)
            .to_lazy()
            .promote();
        for (id, document) in remote_document_changes {
            document_repo::insert(self.config, RepoSource::Base, id, &document);
        }
        for (id, document) in merge_document_changes {
            document_repo::insert(self.config, RepoSource::Local, id, &document);
        }

        Ok(())
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

        // update remote to local (metadata)
        let metadata_changes = self.get_all_metadata_changes()?;
        if !metadata_changes.is_empty() {
            api_service::request(account, FileMetadataUpsertsRequest { updates: metadata_changes })
                .map_err(CoreError::from)?;
        }

        // update base to local
        self.promote_metadata()?;

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
        for id in self.get_all_with_document_changes(config)? {
            let mut local_metadata = self.get_metadata(RepoSource::Local, id)?;
            let local_content =
                file_service::get_document(config, RepoSource::Local, &local_metadata)?;
            let encrypted_content = file_encryption_service::encrypt_document(
                &file_compression_service::compress(&local_content)?,
                &local_metadata,
            )?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local_metadata.decrypted_name.clone()),
            ));

            // update remote to local (document)
            local_metadata.content_version = api_service::request(
                account,
                ChangeDocumentContentRequest {
                    id,
                    old_metadata_version: local_metadata.metadata_version,
                    new_content: encrypted_content,
                },
            )
            .map_err(CoreError::from)?
            .new_content_version;

            // save content version change
            let mut base_metadata = self.get_metadata(RepoSource::Base, id)?;
            base_metadata.content_version = local_metadata.content_version;
            self.insert_metadatum(config, RepoSource::Local, &local_metadata)?;
            self.insert_metadatum(config, RepoSource::Base, &base_metadata)?;
        }

        // update base to local
        self.promote_documents(config)?;

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self, config: &Config) -> CoreResult<WorkCalculated> {
        // fetch metadata updates
        let account = self.get_account()?;
        // todo: if this doesn't need to be mut, prune is broken
        let updates = self.get_updates(account)?;
        let remote_changes = updates.file_metadata;

        // prune prunable files
        {
            let mut local = self
                .tx
                .base_metadata
                .stage(remote_changes)
                .stage(&mut self.tx.local_metadata)
                .to_lazy();
            for id in local.prunable_ids()? {
                local.remove(id);
            }
        }

        // calculate work
        let mut work_units: Vec<WorkUnit> = vec![];
        {
            let remote = self.tx.base_metadata.stage(remote_changes).to_lazy();
            for id in remote.tree.staged.owned_ids() {
                work_units
                    .push(WorkUnit::ServerChange { metadata: remote.finalize(&id, account)? });
            }
        }
        {
            let local = self
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
