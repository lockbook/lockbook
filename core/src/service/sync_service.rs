use crate::model::filename::DocumentType;
use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::pure_functions::files;
use crate::repo::schema::{OneKey, Tx};
use crate::service::{api_service, file_encryption_service, file_service};
use crate::{Config, CoreError};
use lockbook_crypto::clock_service::get_time;
use lockbook_models::account::Account;
use lockbook_models::api::{
    ChangeDocumentContentRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{DecryptedFileMetadata, EncryptedFileMetadata, FileType};
use lockbook_models::tree::FileMetaExt;
use lockbook_models::work_unit::{ClientWorkUnit, WorkUnit};
use serde::Serialize;
use std::fmt;

use super::file_compression_service;

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

#[derive(PartialEq, Debug)]
pub enum MaybeMergeResult<T> {
    Resolved(T),
    Conflict { base: T, local: T, remote: T },
    BaselessConflict { local: T, remote: T },
}

pub fn merge_maybe<T>(
    maybe_base: Option<T>, maybe_local: Option<T>, maybe_remote: Option<T>,
) -> Result<MaybeMergeResult<T>, CoreError> {
    Ok(MaybeMergeResult::Resolved(match (maybe_base, maybe_local, maybe_remote) {
        (None, None, None) => {
            // improper call of this function
            return Err(CoreError::Unexpected(String::from(
                "3-way maybe merge with none of the 3",
            )));
        }
        (None, None, Some(remote)) => {
            // new from remote
            remote
        }
        (None, Some(local), None) => {
            // new from local
            local
        }
        (None, Some(local), Some(remote)) => {
            // Every once in a while, a lockbook client successfully syncs a file to server then gets interrupted
            // before noting the successful sync. The next time that client pushes they're required to pull first
            // and the next time they pull they'll merge the local and remote version of the file with no base.
            // It's possible there have been changes made by other clients in the meantime, but we do the best we
            // can to produce a reasonable result.
            return Ok(MaybeMergeResult::BaselessConflict { local, remote });
        }
        (Some(base), None, None) => {
            // no changes
            base
        }
        (Some(_base), None, Some(remote)) => {
            // remote changes
            remote
        }
        (Some(_base), Some(local), None) => {
            // local changes
            local
        }
        (Some(base), Some(local), Some(remote)) => {
            // conflict
            return Ok(MaybeMergeResult::Conflict { base, local, remote });
        }
    }))
}

pub fn merge_metadata(
    base: DecryptedFileMetadata, local: DecryptedFileMetadata, remote: DecryptedFileMetadata,
) -> DecryptedFileMetadata {
    let local_renamed = local.decrypted_name != base.decrypted_name;
    let remote_renamed = remote.decrypted_name != base.decrypted_name;
    let decrypted_name = match (local_renamed, remote_renamed) {
        (false, false) => base.decrypted_name,
        (true, false) => local.decrypted_name,
        (false, true) => remote.decrypted_name,
        (true, true) => remote.decrypted_name, // resolve rename conflicts in favor of remote
    };

    let local_moved = local.parent != base.parent;
    let remote_moved = remote.parent != base.parent;
    let parent = match (local_moved, remote_moved) {
        (false, false) => base.parent,
        (true, false) => local.parent,
        (false, true) => remote.parent,
        (true, true) => remote.parent, // resolve move conflicts in favor of remote
    };

    DecryptedFileMetadata {
        id: base.id,               // ids never change
        file_type: base.file_type, // file types never change
        parent,
        decrypted_name,
        owner: base.owner,                         // owners never change
        metadata_version: remote.metadata_version, // resolve metadata version conflicts in favor of remote
        content_version: remote.content_version, // resolve content version conflicts in favor of remote
        deleted: base.deleted || local.deleted || remote.deleted, // resolve delete conflicts by deleting
        decrypted_access_key: base.decrypted_access_key,          // access keys never change
    }
}

pub fn merge_maybe_metadata(
    maybe_base: Option<DecryptedFileMetadata>, maybe_local: Option<DecryptedFileMetadata>,
    maybe_remote: Option<DecryptedFileMetadata>,
) -> Result<DecryptedFileMetadata, CoreError> {
    Ok(match merge_maybe(maybe_base, maybe_local, maybe_remote)? {
        MaybeMergeResult::Resolved(merged) => merged,
        MaybeMergeResult::Conflict { base, local, remote } => merge_metadata(base, local, remote),
        MaybeMergeResult::BaselessConflict { local: _local, remote } => remote,
    })
}

fn merge_maybe_documents(
    merged_metadata: &DecryptedFileMetadata, remote_metadata: &DecryptedFileMetadata,
    maybe_base_document: Option<DecryptedDocument>,
    maybe_local_document: Option<DecryptedDocument>, remote_document: DecryptedDocument,
) -> Result<ResolvedDocument, CoreError> {
    Ok(
        match merge_maybe(maybe_base_document, maybe_local_document, Some(remote_document.clone()))?
        {
            MaybeMergeResult::Resolved(merged_document) => ResolvedDocument::Merged {
                remote_metadata: remote_metadata.clone(),
                remote_document,
                merged_metadata: merged_metadata.clone(),
                merged_document,
            },
            MaybeMergeResult::Conflict {
                base: base_document,
                local: local_document,
                remote: remote_document,
            } => {
                match DocumentType::from_file_name_using_extension(&merged_metadata.decrypted_name)
                {
                    // text documents get 3-way merged
                    DocumentType::Text => {
                        let merged_document = match diffy::merge_bytes(
                            &base_document,
                            &local_document,
                            &remote_document,
                        ) {
                            Ok(without_conflicts) => without_conflicts,
                            Err(with_conflicts) => with_conflicts,
                        };
                        ResolvedDocument::Merged {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            merged_metadata: merged_metadata.clone(),
                            merged_document,
                        }
                    }
                    // other documents have local version copied to new file
                    DocumentType::Drawing | DocumentType::Other => {
                        let copied_local_metadata = files::create(
                            FileType::Document,
                            merged_metadata.parent,
                            &merged_metadata.decrypted_name,
                            &merged_metadata.owner.0,
                        );

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata,
                            copied_local_document: local_document,
                        }
                    }
                }
            }
            MaybeMergeResult::BaselessConflict {
                local: local_document,
                remote: remote_document,
            } => {
                match DocumentType::from_file_name_using_extension(&merged_metadata.decrypted_name)
                {
                    // text documents get 3-way merged
                    DocumentType::Text => {
                        let merged_document =
                            match diffy::merge_bytes(&[], &local_document, &remote_document) {
                                Ok(without_conflicts) => without_conflicts,
                                Err(with_conflicts) => with_conflicts,
                            };
                        ResolvedDocument::Merged {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            merged_metadata: merged_metadata.clone(),
                            merged_document,
                        }
                    }
                    // other documents have local version copied to new file
                    DocumentType::Drawing | DocumentType::Other => {
                        let copied_local_metadata = files::create(
                            FileType::Document,
                            merged_metadata.parent,
                            &merged_metadata.decrypted_name,
                            &merged_metadata.owner.0,
                        );

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata,
                            copied_local_document: local_document,
                        }
                    }
                }
            }
        },
    )
}

enum ResolvedDocument {
    Merged {
        remote_metadata: DecryptedFileMetadata,
        remote_document: DecryptedDocument,
        merged_metadata: DecryptedFileMetadata,
        merged_document: DecryptedDocument,
    },
    Copied {
        remote_metadata: DecryptedFileMetadata,
        remote_document: DecryptedDocument,
        copied_local_metadata: DecryptedFileMetadata,
        copied_local_document: DecryptedDocument,
    },
}

impl fmt::Debug for ResolvedDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedDocument::Merged {
                remote_metadata,
                remote_document,
                merged_metadata,
                merged_document,
            } => f
                .debug_struct("ResolvedDocument::Merged")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("merged_metadata", merged_metadata)
                .field("merged_document", &String::from_utf8_lossy(merged_document))
                .finish(),
            ResolvedDocument::Copied {
                remote_metadata,
                remote_document,
                copied_local_metadata,
                copied_local_document,
            } => f
                .debug_struct("ResolvedDocument::Copied")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("copied_local_metadata", copied_local_metadata)
                .field("copied_local_document", &String::from_utf8_lossy(copied_local_document))
                .finish(),
        }
    }
}

/// Gets a resolved document based on merge of local, base, and remote. Some document types are 3-way merged; others
/// have old contents copied to a new file. Remote document is returned so that caller can update base.
#[instrument(level = "debug", skip_all, err(Debug))]
fn get_resolved_document(
    config: &Config, account: &Account, all_metadata_state: &[RepoState<DecryptedFileMetadata>],
    remote_metadatum: &DecryptedFileMetadata, merged_metadatum: &DecryptedFileMetadata,
) -> Result<ResolvedDocument, CoreError> {
    let remote_document = if remote_metadatum.content_version != 0 {
        let remote_document_encrypted =
            api_service::request(account, GetDocumentRequest::from(remote_metadatum))?.content;
        file_compression_service::decompress(&file_encryption_service::decrypt_document(
            &remote_document_encrypted,
            remote_metadatum,
        )?)?
    } else {
        vec![]
    };

    let maybe_metadata_state = all_metadata_state
        .iter()
        .find(|&f| f.clone().local().id == remote_metadatum.id);
    let maybe_document_state = if let Some(metadata_state) = maybe_metadata_state {
        file_service::maybe_get_document_state(config, metadata_state)?
    } else {
        None
    };

    let (maybe_local_document, maybe_base_document) =
        if let Some(document_state) = maybe_document_state {
            match document_state {
                RepoState::New(local) => (Some(local), None),
                RepoState::Unmodified(base) => (None, Some(base)),
                RepoState::Modified { local, base } => (Some(local), Some(base)),
            }
        } else {
            (None, None)
        };

    // merge document content for documents with updated content
    let mut merged_document = merge_maybe_documents(
        merged_metadatum,
        remote_metadatum,
        maybe_base_document,
        maybe_local_document,
        remote_document,
    )?;

    if let ResolvedDocument::Copied {
        remote_metadata: _,
        remote_document: _,
        ref mut copied_local_metadata,
        copied_local_document: _,
    } = merged_document
    {
        copied_local_metadata.decrypted_name = files::suggest_non_conflicting_filename(
            copied_local_metadata.id,
            &all_metadata_state
                .iter()
                .map(|rs| rs.clone().local())
                .collect::<Vec<DecryptedFileMetadata>>(),
            &[copied_local_metadata.clone()],
        )?;
    }

    Ok(merged_document)
}

fn should_pull_document(
    maybe_base: &Option<DecryptedFileMetadata>, maybe_local: &Option<DecryptedFileMetadata>,
    maybe_remote: &Option<DecryptedFileMetadata>,
) -> bool {
    if let Some(remote) = maybe_remote {
        remote.file_type == FileType::Document
            && if let Some(local) = maybe_local {
                remote.content_version > local.content_version
            } else if let Some(base) = maybe_base {
                remote.content_version > base.content_version
            } else {
                true
            }
            && !remote.deleted
    } else {
        false
    }
}

enum SyncProgressOperation {
    IncrementTotalWork(usize),
    StartWorkUnit(ClientWorkUnit),
}

impl Tx<'_> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync<F: Fn(SyncProgress)>(
        &mut self, config: &Config, maybe_update_sync_progress: Option<F>,
    ) -> Result<(), CoreError> {
        let mut sync_progress_total = 4 + self.get_all_with_document_changes(config)?.len(); // 3 metadata pulls + 1 metadata push + doc pushes
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
        self.prune_deleted(config)?;
        self.push_metadata(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.push_documents(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.prune_deleted(config)?;
        self.last_synced.insert(OneKey {}, get_time().0);
        Ok(())
    }

    /// Updates local files to 3-way merge of local, base, and remote; updates base files to remote.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn pull<F>(&mut self, config: &Config, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = &self.get_account()?;
        let base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let base_max_metadata_version = base_metadata
            .iter()
            .map(|f| f.metadata_version)
            .max()
            .unwrap_or(0);

        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PullMetadata));

        let remote_metadata_changes = api_service::request(
            account,
            GetUpdatesRequest { since_metadata_version: base_max_metadata_version },
        )
        .map_err(CoreError::from)?
        .file_metadata;

        let local_metadata = self.get_all_metadata(RepoSource::Local)?;
        let (remote_metadata, remote_orphans) = self
            .get_all_metadata_with_encrypted_changes(RepoSource::Base, &remote_metadata_changes)?;
        let all_metadata_state = self.get_all_metadata_state()?;

        let num_documents_to_pull = remote_metadata_changes
            .iter()
            .filter(|&f| {
                let maybe_remote_metadatum = remote_metadata.maybe_find(f.id);
                let maybe_base_metadatum = base_metadata.maybe_find(f.id);
                let maybe_local_metadatum = local_metadata.maybe_find(f.id);
                should_pull_document(
                    &maybe_base_metadatum,
                    &maybe_local_metadatum,
                    &maybe_remote_metadatum,
                )
            })
            .count();
        update_sync_progress(SyncProgressOperation::IncrementTotalWork(num_documents_to_pull));

        let mut base_metadata_updates = Vec::new();
        let mut base_document_updates = Vec::new();
        let mut local_metadata_updates = Vec::new();
        let mut local_document_updates = Vec::new();

        // iterate changes
        for encrypted_remote_metadatum in remote_metadata_changes {
            // skip filtered changes
            if remote_metadata
                .maybe_find(encrypted_remote_metadatum.id)
                .is_none()
            {
                continue;
            }

            // merge metadata
            let remote_metadatum = remote_metadata.find(encrypted_remote_metadatum.id)?;
            let maybe_base_metadatum = base_metadata.maybe_find(encrypted_remote_metadatum.id);
            let maybe_local_metadatum = local_metadata.maybe_find(encrypted_remote_metadatum.id);

            let merged_metadatum = merge_maybe_metadata(
                maybe_base_metadatum.clone(),
                maybe_local_metadatum.clone(),
                Some(remote_metadatum.clone()),
            )?;
            base_metadata_updates.push(remote_metadatum.clone()); // update base to remote
            local_metadata_updates.push(merged_metadatum.clone()); // update local to merged

            // merge document content
            if should_pull_document(
                &maybe_base_metadatum,
                &maybe_local_metadatum,
                &Some(remote_metadatum.clone()),
            ) {
                update_sync_progress(SyncProgressOperation::StartWorkUnit(
                    ClientWorkUnit::PullDocument(remote_metadatum.decrypted_name.clone()),
                ));

                match get_resolved_document(
                    config,
                    account,
                    &all_metadata_state,
                    &remote_metadatum,
                    &merged_metadatum,
                )? {
                    ResolvedDocument::Merged {
                        remote_metadata,
                        remote_document,
                        merged_metadata,
                        merged_document,
                    } => {
                        // update base to remote
                        base_document_updates.push((remote_metadata, remote_document));
                        // update local to merged
                        local_document_updates.push((merged_metadata, merged_document));
                    }
                    ResolvedDocument::Copied {
                        remote_metadata,
                        remote_document,
                        copied_local_metadata,
                        copied_local_document,
                    } => {
                        base_document_updates
                            .push((remote_metadata.clone(), remote_document.clone())); // update base to remote
                        local_metadata_updates.push(remote_metadata.clone()); // reset conflicted local
                        local_document_updates.push((remote_metadata.clone(), remote_document)); // reset conflicted local
                        local_metadata_updates.push(copied_local_metadata.clone()); // new local metadata from merge
                        local_document_updates
                            .push((copied_local_metadata.clone(), copied_local_document));
                        // new local document from merge
                    }
                }
            }
        }

        // deleted orphaned updates
        for orphan in remote_orphans {
            if let Some(mut metadatum) = base_metadata.maybe_find(orphan.id) {
                if let Some(mut metadatum_update) = base_metadata_updates.maybe_find_mut(orphan.id)
                {
                    metadatum_update.deleted = true;
                } else {
                    metadatum.deleted = true;
                    base_metadata_updates.push(metadatum);
                }
            }
            if let Some(mut metadatum) = local_metadata.maybe_find(orphan.id) {
                if let Some(mut metadatum_update) = local_metadata_updates.maybe_find_mut(orphan.id)
                {
                    metadatum_update.deleted = true;
                } else {
                    metadatum.deleted = true;
                    local_metadata_updates.push(metadatum);
                }
            }
        }

        // resolve cycles
        for self_descendant in local_metadata.get_invalid_cycles(&local_metadata_updates)? {
            if let Some(RepoState::Modified { mut local, base }) =
                self.maybe_get_metadata_state(self_descendant)?
            {
                if local.parent != base.parent {
                    if let Some(existing_update) =
                        local_metadata_updates.maybe_find_mut(self_descendant)
                    {
                        existing_update.parent = base.parent;
                    } else {
                        local.parent = base.parent;
                        local_metadata_updates.push(local);
                    }
                }
            }
        }

        // resolve path conflicts
        for path_conflict in local_metadata.get_path_conflicts(&local_metadata_updates)? {
            let local_meta_updates_copy = local_metadata_updates.clone();

            let conflict_name = files::suggest_non_conflicting_filename(
                path_conflict.existing,
                &local_metadata,
                &local_meta_updates_copy,
            )?;
            if let Some(existing_update) =
                local_metadata_updates.maybe_find_mut(path_conflict.existing)
            {
                existing_update.decrypted_name = conflict_name;
            } else {
                let mut new_metadatum_update = local_metadata.find(path_conflict.existing)?;
                new_metadatum_update.decrypted_name = conflict_name;
                local_metadata_updates.push(new_metadatum_update);
            }
        }

        // update metadata
        self.insert_metadata_both_repos(config, &base_metadata_updates, &local_metadata_updates)?;

        // update document content
        for (metadata, document_update) in base_document_updates {
            self.insert_document(config, RepoSource::Base, &metadata, &document_update)?;
        }
        for (metadata, document_update) in local_document_updates {
            self.insert_document(config, RepoSource::Local, &metadata, &document_update)?;
        }

        Ok(())
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

    pub fn calculate_work(&self, config: &Config) -> Result<WorkCalculated, CoreError> {
        let account = &self.get_account()?;
        let base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let base_max_metadata_version = base_metadata
            .iter()
            .map(|f| f.metadata_version)
            .max()
            .unwrap_or(0);

        let server_updates = api_service::request(
            account,
            GetUpdatesRequest { since_metadata_version: base_max_metadata_version },
        )
        .map_err(CoreError::from)?
        .file_metadata;

        self.calculate_work_from_updates(config, &server_updates, base_max_metadata_version)
    }

    fn calculate_work_from_updates(
        &self, config: &Config, server_updates: &[EncryptedFileMetadata], mut last_sync: u64,
    ) -> Result<WorkCalculated, CoreError> {
        let mut work_units: Vec<WorkUnit> = vec![];
        let (all_metadata, _) =
            self.get_all_metadata_with_encrypted_changes(RepoSource::Local, server_updates)?;
        for metadata in server_updates {
            // skip filtered changes
            if all_metadata.maybe_find(metadata.id).is_none() {
                continue;
            }

            if metadata.metadata_version > last_sync {
                last_sync = metadata.metadata_version;
            }

            match self.maybe_get_metadata(RepoSource::Local, metadata.id)? {
                None => {
                    if !metadata.deleted {
                        // no work for files we don't have that have been deleted
                        work_units.push(WorkUnit::ServerChange {
                            metadata: all_metadata.find(metadata.id)?,
                        })
                    }
                }
                Some(local_metadata) => {
                    if metadata.metadata_version != local_metadata.metadata_version {
                        work_units.push(WorkUnit::ServerChange {
                            metadata: all_metadata.find(metadata.id)?,
                        })
                    }
                }
            };
        }

        work_units.sort_by(|f1, f2| {
            f1.get_metadata()
                .metadata_version
                .cmp(&f2.get_metadata().metadata_version)
        });

        for file_diff in self.get_all_metadata_changes()? {
            let metadata = self.get_metadata(RepoSource::Local, file_diff.id)?;
            work_units.push(WorkUnit::LocalChange { metadata });
        }
        for doc_id in self.get_all_with_document_changes(config)? {
            let metadata = self.get_metadata(RepoSource::Local, doc_id)?;
            work_units.push(WorkUnit::LocalChange { metadata });
        }

        Ok(WorkCalculated { work_units, most_recent_update_from_server: last_sync })
    }
}
