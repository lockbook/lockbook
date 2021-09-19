use crate::model::client_conversion::ClientWorkUnit;
use crate::model::document_type::DocumentType;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::file_repo;
use crate::repo::last_updated_repo;
use crate::service::{file_encryption_service, file_service};
use crate::CoreError;
use crate::{client, utils};
use lockbook_models::account::Account;
use lockbook_models::api::{
    ChangeDocumentContentRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use lockbook_models::work_unit::WorkUnit;
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

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, CoreError> {
    info!("Calculating Work");

    let account = account_repo::get(config)?;
    let last_sync = last_updated_repo::get(config)?;

    let server_updates = client::request(
        &account,
        GetUpdatesRequest {
            since_metadata_version: last_sync,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    calculate_work_from_updates(config, &server_updates, last_sync)
}

fn calculate_work_from_updates(
    config: &Config,
    server_updates: &[FileMetadata],
    last_sync: u64,
) -> Result<WorkCalculated, CoreError> {
    let mut most_recent_update_from_server: u64 = last_sync;
    let mut work_units: Vec<WorkUnit> = vec![];
    let all_metadata = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Local,
        server_updates,
    )?;
    for metadata in server_updates {
        if metadata.metadata_version > most_recent_update_from_server {
            most_recent_update_from_server = metadata.metadata_version;
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

    let changes = file_repo::get_all_metadata_changes(config)?;
    for change_description in changes {
        let metadata = file_repo::get_metadata(config, RepoSource::Local, change_description.id)?;
        work_units.push(WorkUnit::LocalChange { metadata });
    }
    debug!("Work Calculated: {:#?}", work_units);

    Ok(WorkCalculated {
        work_units,
        most_recent_update_from_server,
    })
}

pub enum MaybeMergeResult<T> {
    Resolved(T),
    Conflict { base: T, local: T, remote: T },
}

fn maybe_merge<T>(
    maybe_base: Option<T>,
    maybe_local: Option<T>,
    maybe_remote: Option<T>,
) -> Result<MaybeMergeResult<T>, CoreError> {
    Ok(MaybeMergeResult::Resolved(
        match (maybe_base, maybe_local, maybe_remote) {
            (None, None, None) => {
                // improper call of this function
                return Err(CoreError::Unexpected(String::from(
                    "3-way metadata merge with none of the 3",
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
            (None, Some(_local), Some(_remote)) => {
                // new from local and from remote with same id - bug
                return Err(CoreError::Unexpected(String::from(
                    "new local file with same id as new remote file",
                )));
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
    let remote_moved = remote.parent != remote.parent;
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
    Ok(match maybe_merge(maybe_base, maybe_local, maybe_remote)? {
        MaybeMergeResult::Resolved(merged) => merged,
        MaybeMergeResult::Conflict {
            base,
            local,
            remote,
        } => merge_metadata(base, local, remote),
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
        match maybe_merge(
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
                        let mut copied_local_metadata = file_service::create(
                        FileType::Document,
                        merged_metadata.parent,
                        "this is overwritten two statements down because we need the uuid generated by this function call",
                        &merged_metadata.owner,
                    );
                        let conflict_name = format!(
                            "{}-CONTENT-CONFLICT-{}",
                            copied_local_metadata.id.clone(),
                            &merged_metadata.decrypted_name,
                        );
                        copied_local_metadata.decrypted_name = conflict_name;

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata: merged_metadata.clone(),
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

/// Gets a resolved document based on merge of local, base, and remote. Some document types are 3-way merged; others
/// have old contents copied to a new file. Remote document is returned so that caller can update base.
fn get_resolved_document(
    config: &Config,
    account: &Account,
    remote_metadatum: &DecryptedFileMetadata,
    merged_metadatum: &DecryptedFileMetadata,
) -> Result<ResolvedDocument, CoreError> {
    let maybe_remote_document_encrypted = client::request(
        &account,
        GetDocumentRequest {
            id: remote_metadatum.id,
            content_version: remote_metadatum.content_version,
        },
    )?
    .content;
    let remote_document = match maybe_remote_document_encrypted {
        Some(remote_document_encrypted) => file_encryption_service::decrypt_document(
            &remote_document_encrypted,
            &remote_metadatum,
        )?,
        None => Vec::new(),
    };
    let maybe_base_document =
        file_repo::maybe_get_document(config, RepoSource::Base, remote_metadatum.id)?;
    let maybe_local_document =
        file_repo::maybe_get_document(config, RepoSource::Local, remote_metadatum.id)?;

    // update remote repo to version from server
    file_repo::insert_document(
        config,
        RepoSource::Base,
        &remote_metadatum,
        &remote_document,
    )?;

    // merge document content for documents with updated content
    let merged_document = merge_maybe_documents(
        merged_metadatum,
        remote_metadatum,
        maybe_base_document,
        maybe_local_document,
        remote_document,
    )?;

    Ok(merged_document)
}

/// Updates local files to 3-way merge of local, base, and remote; updates base files to remote.
fn pull(
    config: &Config,
    account: &Account,
    f: &Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    let last_sync = last_updated_repo::get(config)?;
    let remote_metadata_changes = client::request(
        account,
        GetUpdatesRequest {
            since_metadata_version: last_sync,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    let local_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let base_metadata = file_repo::get_all_metadata(config, RepoSource::Base)?;
    let remote_metadata = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Base,
        &remote_metadata_changes,
    )?;

    let mut local_metadata_updates = Vec::new();
    let mut local_document_updates = Vec::new();
    let mut base_metadata_updates = Vec::new();
    let mut base_document_updates = Vec::new();

    // iterate changes
    for encrypted_remote_metadatum in remote_metadata_changes {
        // merge metadata
        let remote_metadatum = utils::find(&remote_metadata, encrypted_remote_metadatum.id)?;
        let maybe_base_metadatum = utils::maybe_find(&base_metadata, encrypted_remote_metadatum.id);
        let maybe_local_metadatum =
            utils::maybe_find(&local_metadata, encrypted_remote_metadatum.id);

        let merged_metadatum = merge_maybe_metadata(
            maybe_base_metadatum.clone(),
            maybe_local_metadatum,
            Some(remote_metadatum.clone()),
        )?;
        local_metadata_updates.push(merged_metadatum.clone()); // update local to merged
        base_metadata_updates.push(remote_metadatum.clone()); // update base to remote

        // merge document content
        let content_updated = remote_metadatum.file_type == FileType::Document
            && if let Some(base) = maybe_base_metadatum {
                remote_metadatum.content_version != base.content_version
            } else {
                true
            };
        if content_updated {
            match get_resolved_document(config, account, &remote_metadatum, &merged_metadatum)? {
                ResolvedDocument::Merged {
                    remote_metadata,
                    remote_document,
                    merged_metadata,
                    merged_document,
                } => {
                    local_document_updates.push((merged_metadata, merged_document)); // update local to merged
                    base_document_updates.push((remote_metadata, remote_document));
                    // update base to remote
                }
                ResolvedDocument::Copied {
                    remote_metadata,
                    remote_document,
                    copied_local_metadata,
                    copied_local_document,
                } => {
                    local_metadata_updates.push(copied_local_metadata.clone()); // new local file from merge
                    local_document_updates.push((copied_local_metadata, copied_local_document));
                    base_document_updates.push((remote_metadata, remote_document));
                    // update base to remote
                }
            }
        }
    }

    // resolve path conflicts
    for path_conflict in file_service::get_path_conflicts(&local_metadata, &local_metadata_updates)?
    {
        let to_rename = utils::find_mut(&mut local_metadata_updates, path_conflict.staged)?;
        let conflict_name = format!(
            "{}-PATH-CONFLICT-{}",
            to_rename.id.clone(),
            &to_rename.decrypted_name,
        );
        to_rename.decrypted_name = conflict_name;
    }

    // update local
    for metadata_update in local_metadata_updates {
        file_repo::insert_metadata(config, RepoSource::Local, &metadata_update)?;
    }
    for (metadata, document_update) in local_document_updates {
        file_repo::insert_document(config, RepoSource::Local, &metadata, &document_update)?;
    }

    // update base
    for metadata_update in base_metadata_updates {
        file_repo::insert_metadata(config, RepoSource::Base, &metadata_update)?;
    }
    for (metadata, document_update) in base_document_updates {
        file_repo::insert_document(config, RepoSource::Base, &metadata, &document_update)?;
    }

    Ok(())
}

/// Updates remote and base files to local.
fn push(
    config: &Config,
    account: &Account,
    f: &Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    for id in file_repo::get_all_with_document_changes(config)? {
        let local_metadata = file_repo::get_metadata(config, RepoSource::Local, id)?;
        let local_content = file_repo::get_document(config, RepoSource::Local, id)?;
        let encrypted_content =
            file_encryption_service::encrypt_document(&local_content, &local_metadata)?;

        // update remote to local (document)
        client::request(
            &account,
            ChangeDocumentContentRequest {
                id: id,
                old_metadata_version: local_metadata.metadata_version,
                new_content: encrypted_content,
            },
        )
        .map_err(CoreError::from)?;
    }

    // update remote to local (metadata)
    client::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: file_repo::get_all_metadata_changes(config)?,
        },
    )
    .map_err(CoreError::from)?;

    // update base to local
    file_repo::promote(config)?;

    Ok(())
}

pub fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), CoreError> {
    let account = &account_repo::get(config)?;
    pull(config, account, &f)?;
    push(config, account, &f)?;
    file_repo::prune_deleted(config)?;
    Ok(())
}
