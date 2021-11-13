use std::fmt;

use crate::model::client_conversion::ClientWorkUnit;
use crate::model::filename::DocumentType;
use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::{file_repo, last_updated_repo};
use crate::service::{api_service, file_encryption_service, file_service};
use crate::utils;
use crate::CoreError;
use lockbook_models::account::Account;
use lockbook_models::api::{
    ChangeDocumentContentRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use lockbook_models::work_unit::WorkUnit;
use serde::Serialize;

use super::file_compression_service;
use lockbook_crypto::clock_service::get_time;

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

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, CoreError> {
    info!("Calculating Work");

    let account = account_repo::get(config)?;
    let base_metadata = file_repo::get_all_metadata(config, RepoSource::Base)?;
    let base_max_metadata_version = base_metadata
        .iter()
        .map(|f| f.metadata_version)
        .max()
        .unwrap_or(0);

    let server_updates = api_service::request(
        &account,
        GetUpdatesRequest {
            since_metadata_version: base_max_metadata_version,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    calculate_work_from_updates(config, &server_updates, base_max_metadata_version)
}

fn calculate_work_from_updates(
    config: &Config,
    server_updates: &[FileMetadata],
    mut last_sync: u64,
) -> Result<WorkCalculated, CoreError> {
    let mut work_units: Vec<WorkUnit> = vec![];
    let (all_metadata, _) = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Local,
        server_updates,
    )?;
    for metadata in server_updates {
        // skip filtered changes
        if utils::maybe_find(&all_metadata, metadata.id).is_none() {
            continue;
        }

        if metadata.metadata_version > last_sync {
            last_sync = metadata.metadata_version;
        }

        match file_repo::maybe_get_metadata(config, RepoSource::Local, metadata.id)? {
            None => {
                if !metadata.deleted {
                    // no work for files we don't have that have been deleted
                    work_units.push(WorkUnit::ServerChange {
                        metadata: utils::find(&all_metadata, metadata.id)?,
                    })
                }
            }
            Some(local_metadata) => {
                if metadata.metadata_version != local_metadata.metadata_version {
                    work_units.push(WorkUnit::ServerChange {
                        metadata: utils::find(&all_metadata, metadata.id)?,
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

    for file_diff in file_repo::get_all_metadata_changes(config)? {
        let metadata = file_repo::get_metadata(config, RepoSource::Local, file_diff.id)?;
        work_units.push(WorkUnit::LocalChange { metadata });
    }
    for doc_id in file_repo::get_all_with_document_changes(config)? {
        let metadata = file_repo::get_metadata(config, RepoSource::Local, doc_id)?;
        work_units.push(WorkUnit::LocalChange { metadata });
    }
    debug!("Work Calculated: {:#?}", work_units);

    Ok(WorkCalculated {
        work_units,
        most_recent_update_from_server: last_sync,
    })
}

#[derive(PartialEq, Debug)]
pub enum MaybeMergeResult<T> {
    Resolved(T),
    Conflict { base: T, local: T, remote: T },
    BaselessConflict { local: T, remote: T },
}

fn merge_maybe<T>(
    maybe_base: Option<T>,
    maybe_local: Option<T>,
    maybe_remote: Option<T>,
) -> Result<MaybeMergeResult<T>, CoreError> {
    Ok(MaybeMergeResult::Resolved(
        match (maybe_base, maybe_local, maybe_remote) {
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
                return Ok(MaybeMergeResult::Conflict {
                    base,
                    local,
                    remote,
                });
            }
        },
    ))
}

fn merge_metadata(
    base: DecryptedFileMetadata,
    local: DecryptedFileMetadata,
    remote: DecryptedFileMetadata,
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

fn merge_maybe_metadata(
    maybe_base: Option<DecryptedFileMetadata>,
    maybe_local: Option<DecryptedFileMetadata>,
    maybe_remote: Option<DecryptedFileMetadata>,
) -> Result<DecryptedFileMetadata, CoreError> {
    Ok(match merge_maybe(maybe_base, maybe_local, maybe_remote)? {
        MaybeMergeResult::Resolved(merged) => merged,
        MaybeMergeResult::Conflict {
            base,
            local,
            remote,
        } => merge_metadata(base, local, remote),
        MaybeMergeResult::BaselessConflict {
            local: _local,
            remote,
        } => remote,
    })
}

fn merge_maybe_documents(
    merged_metadata: &DecryptedFileMetadata,
    remote_metadata: &DecryptedFileMetadata,
    maybe_base_document: Option<DecryptedDocument>,
    maybe_local_document: Option<DecryptedDocument>,
    remote_document: DecryptedDocument,
) -> Result<ResolvedDocument, CoreError> {
    Ok(
        match merge_maybe(
            maybe_base_document,
            maybe_local_document,
            Some(remote_document.clone()),
        )? {
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
                        let copied_local_metadata = file_service::create(
                            FileType::Document,
                            merged_metadata.parent,
                            &merged_metadata.decrypted_name,
                            &merged_metadata.owner,
                        );

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata: copied_local_metadata,
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
                        let copied_local_metadata = file_service::create(
                            FileType::Document,
                            merged_metadata.parent,
                            &merged_metadata.decrypted_name,
                            &merged_metadata.owner,
                        );

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata: copied_local_metadata,
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
                .field(
                    "copied_local_document",
                    &String::from_utf8_lossy(copied_local_document),
                )
                .finish(),
        }
    }
}

/// Gets a resolved document based on merge of local, base, and remote. Some document types are 3-way merged; others
/// have old contents copied to a new file. Remote document is returned so that caller can update base.
fn get_resolved_document(
    config: &Config,
    account: &Account,
    all_metadata_state: &[RepoState<DecryptedFileMetadata>],
    remote_metadatum: &DecryptedFileMetadata,
    merged_metadatum: &DecryptedFileMetadata,
) -> Result<Option<ResolvedDocument>, CoreError> {
    let maybe_remote_document_encrypted = api_service::request(
        account,
        GetDocumentRequest {
            id: remote_metadatum.id,
            content_version: remote_metadatum.content_version, // todo: is this content_version is incorrect?
        },
    )?
    .content;
    let maybe_remote_document = match maybe_remote_document_encrypted {
        Some(remote_document_encrypted) => Some(file_compression_service::decompress(
            &file_encryption_service::decrypt_document(
                &remote_document_encrypted,
                remote_metadatum,
            )?,
        )?),
        None => None,
    };

    let maybe_metadata_state = all_metadata_state
        .iter()
        .find(|&f| f.clone().local().id == remote_metadatum.id);
    let maybe_document_state = if let Some(metadata_state) = maybe_metadata_state {
        file_repo::maybe_get_document_state(config, metadata_state)?
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

    match maybe_remote_document {
        Some(remote_document) => {
            // merge document content for documents with updated content
            let merged_document = merge_maybe_documents(
                merged_metadatum,
                remote_metadatum,
                maybe_base_document,
                maybe_local_document,
                remote_document,
            )?;

            Ok(Some(merged_document))
        }
        None => Ok(None),
    }
}

fn should_pull_document(
    maybe_base: &Option<DecryptedFileMetadata>,
    maybe_local: &Option<DecryptedFileMetadata>,
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

/// Updates local files to 3-way merge of local, base, and remote; updates base files to remote.
fn pull<F>(
    config: &Config,
    account: &Account,
    update_sync_progress: &mut F,
) -> Result<(), CoreError>
where
    F: FnMut(SyncProgressOperation),
{
    let base_metadata = file_repo::get_all_metadata(config, RepoSource::Base)?;
    let base_max_metadata_version = base_metadata
        .iter()
        .map(|f| f.metadata_version)
        .max()
        .unwrap_or(0);

    update_sync_progress(SyncProgressOperation::StartWorkUnit(
        ClientWorkUnit::PullMetadata,
    ));

    let remote_metadata_changes = api_service::request(
        account,
        GetUpdatesRequest {
            since_metadata_version: base_max_metadata_version,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    let local_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let (remote_metadata, remote_orphans) = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Base,
        &remote_metadata_changes,
    )?;
    let all_metadata_state = file_repo::get_all_metadata_state(config)?;

    let num_documents_to_pull = remote_metadata_changes
        .iter()
        .filter(|&f| {
            let maybe_remote_metadatum = utils::maybe_find(&remote_metadata, f.id);
            let maybe_base_metadatum = utils::maybe_find(&base_metadata, f.id);
            let maybe_local_metadatum = utils::maybe_find(&local_metadata, f.id);
            should_pull_document(
                &maybe_base_metadatum,
                &maybe_local_metadatum,
                &maybe_remote_metadatum,
            )
        })
        .count();
    update_sync_progress(SyncProgressOperation::IncrementTotalWork(
        num_documents_to_pull,
    ));

    let mut base_metadata_updates = Vec::new();
    let mut base_document_updates = Vec::new();
    let mut local_metadata_updates = Vec::new();
    let mut local_document_updates = Vec::new();

    // iterate changes
    for encrypted_remote_metadatum in remote_metadata_changes {
        // skip filtered changes
        if utils::maybe_find(&remote_metadata, encrypted_remote_metadatum.id).is_none() {
            continue;
        }

        // merge metadata
        let remote_metadatum = utils::find(&remote_metadata, encrypted_remote_metadatum.id)?;
        let maybe_base_metadatum = utils::maybe_find(&base_metadata, encrypted_remote_metadatum.id);
        let maybe_local_metadatum =
            utils::maybe_find(&local_metadata, encrypted_remote_metadatum.id);

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
                Some(ResolvedDocument::Merged {
                    remote_metadata,
                    remote_document,
                    merged_metadata,
                    merged_document,
                }) => {
                    // update base to remote
                    base_document_updates.push((remote_metadata, remote_document));
                    // update local to merged
                    local_document_updates.push((merged_metadata, merged_document));
                }
                Some(ResolvedDocument::Copied {
                    remote_metadata,
                    remote_document,
                    copied_local_metadata,
                    copied_local_document,
                }) => {
                    base_document_updates.push((remote_metadata.clone(), remote_document.clone())); // update base to remote
                    local_metadata_updates.push(remote_metadata.clone()); // reset conflicted local
                    local_document_updates.push((remote_metadata.clone(), remote_document)); // reset conflicted local
                    local_metadata_updates.push(copied_local_metadata.clone()); // new local metadata from merge
                    local_document_updates
                        .push((copied_local_metadata.clone(), copied_local_document));
                    // new local document from merge
                }
                None => {}
            }
        }
    }

    // deleted orphaned updates
    for orphan in remote_orphans {
        if let Some(mut metadatum) = utils::maybe_find(&base_metadata, orphan.id) {
            if let Some(mut metadatum_update) =
                utils::maybe_find_mut(&mut base_metadata_updates, orphan.id)
            {
                metadatum_update.deleted = true;
            } else {
                metadatum.deleted = true;
                base_metadata_updates.push(metadatum);
            }
        }
        if let Some(mut metadatum) = utils::maybe_find(&local_metadata, orphan.id) {
            if let Some(mut metadatum_update) =
                utils::maybe_find_mut(&mut local_metadata_updates, orphan.id)
            {
                metadatum_update.deleted = true;
            } else {
                metadatum.deleted = true;
                local_metadata_updates.push(metadatum);
            }
        }
    }

    // resolve path conflicts
    for path_conflict in file_service::get_path_conflicts(&local_metadata, &local_metadata_updates)?
    {
        let local_meta_updates_copy = local_metadata_updates.clone();
        let to_rename = utils::find_mut(&mut local_metadata_updates, path_conflict.staged)?;
        let conflict_name = file_service::suggest_non_conflicting_filename(
            to_rename.id,
            &local_metadata,
            &local_meta_updates_copy,
        )?;
        to_rename.decrypted_name = conflict_name;
    }

    // resolve cycles
    for self_descendant in
        file_service::get_invalid_cycles(&local_metadata, &local_metadata_updates)?
    {
        if let Some(RepoState::Modified { mut local, base }) =
            file_repo::maybe_get_metadata_state(config, self_descendant)?
        {
            if let Some(existing_update) =
                utils::maybe_find_mut(&mut local_metadata_updates, self_descendant)
            {
                existing_update.parent = base.parent;
            }
            local.parent = base.parent;
            file_repo::insert_metadatum(config, RepoSource::Local, &local)?;
        }
    }

    // update base
    file_repo::insert_metadata(config, RepoSource::Base, &base_metadata_updates)?;
    for (metadata, document_update) in base_document_updates {
        file_repo::insert_document(config, RepoSource::Base, &metadata, &document_update)?;
    }

    // update local
    file_repo::insert_metadata(config, RepoSource::Local, &local_metadata_updates)?;
    for (metadata, document_update) in local_document_updates {
        file_repo::insert_document(config, RepoSource::Local, &metadata, &document_update)?;
    }

    Ok(())
}

/// Updates remote and base metadata to local.
fn push_metadata<F>(
    config: &Config,
    account: &Account,
    update_sync_progress: &mut F,
) -> Result<(), CoreError>
where
    F: FnMut(SyncProgressOperation),
{
    update_sync_progress(SyncProgressOperation::StartWorkUnit(
        ClientWorkUnit::PushMetadata,
    ));

    // update remote to local (metadata)
    let metadata_changes = file_repo::get_all_metadata_changes(config)?;
    if !metadata_changes.is_empty() {
        api_service::request(
            account,
            FileMetadataUpsertsRequest {
                updates: metadata_changes,
            },
        )
        .map_err(CoreError::from)?;
    }

    // update base to local
    file_repo::promote_metadata(config)?;

    Ok(())
}

/// Updates remote and base files to local.
fn push_documents<F>(
    config: &Config,
    account: &Account,
    update_sync_progress: &mut F,
) -> Result<(), CoreError>
where
    F: FnMut(SyncProgressOperation),
{
    for id in file_repo::get_all_with_document_changes(config)? {
        let mut local_metadata = file_repo::get_metadata(config, RepoSource::Local, id)?;
        let local_content = file_repo::get_document(config, RepoSource::Local, &local_metadata)?;
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
                id: id,
                old_metadata_version: local_metadata.metadata_version,
                new_content: encrypted_content,
            },
        )
        .map_err(CoreError::from)?
        .new_content_version;

        // save content version change
        let mut base_metadata = file_repo::get_metadata(config, RepoSource::Base, id)?;
        base_metadata.content_version = local_metadata.content_version;
        file_repo::insert_metadatum(config, RepoSource::Local, &local_metadata)?;
        file_repo::insert_metadatum(config, RepoSource::Base, &base_metadata)?;
    }

    // update base to local
    file_repo::promote_documents(config)?;

    Ok(())
}

enum SyncProgressOperation {
    IncrementTotalWork(usize),
    StartWorkUnit(ClientWorkUnit),
}

pub fn sync(
    config: &Config,
    maybe_update_sync_progress: Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    let mut sync_progress_total = 4 + file_repo::get_all_with_document_changes(config)?.len(); // 3 metadata pulls + 1 metadata push + doc pushes
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

    let account = &account_repo::get(config)?;
    pull(config, account, &mut update_sync_progress)?;
    file_repo::prune_deleted(config)?;
    push_metadata(config, account, &mut update_sync_progress)?;
    pull(config, account, &mut update_sync_progress)?;
    push_documents(config, account, &mut update_sync_progress)?;
    pull(config, account, &mut update_sync_progress)?;
    file_repo::prune_deleted(config)?;
    last_updated_repo::set(config, get_time().0)?;
    Ok(())
}

#[cfg(test)]
mod unit_test_sync_service {
    use std::str::FromStr;

    use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
    use uuid::Uuid;

    use crate::service::sync_service::{self, MaybeMergeResult};

    #[test]
    fn merge_maybe_resolved_base() {
        let base = Some(0);
        let local = None;
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(0));
    }

    #[test]
    fn merge_maybe_resolved_local() {
        let base = None;
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_local_with_base() {
        let base = Some(0);
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_remote() {
        let base = None;
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_remote_with_base() {
        let base = Some(0);
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_conflict() {
        let base = Some(0);
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(
            result,
            MaybeMergeResult::Conflict {
                base: 0,
                local: 1,
                remote: 2,
            }
        );
    }

    #[test]
    fn merge_maybe_resolved_baseless_conflict() {
        let base = None;
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(
            result,
            MaybeMergeResult::BaselessConflict {
                local: 1,
                remote: 2,
            }
        );
    }

    #[test]
    fn merge_maybe_none() {
        let base = None;
        let local = None;
        let remote = None;

        sync_service::merge_maybe::<i32>(base, local, remote).unwrap_err();
    }

    #[test]
    fn merge_metadata_local_and_remote_moved() {
        let base = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };
        let local = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };
        let remote = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };

        let result = sync_service::merge_metadata(base, local, remote);

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Default::default(),
                decrypted_access_key: Default::default(),
            }
        );
    }

    #[test]
    fn merge_maybe_metadata_local_and_remote_moved() {
        let base = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });
        let local = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });
        let remote = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });

        let result = sync_service::merge_maybe_metadata(base, local, remote).unwrap();

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Default::default(),
                decrypted_access_key: Default::default(),
            }
        );
    }
}
