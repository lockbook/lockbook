use crate::client;
use crate::model::client_conversion::{generate_client_work_unit, ClientWorkUnit};
use crate::model::document_type::DocumentType;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::file_repo;
use crate::repo::last_updated_repo;
use crate::service::{file_encryption_service, file_service};
use crate::CoreError;
use lockbook_models::api::{
    ChangeDocumentContentRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
};
use lockbook_models::file_metadata::{FileMetadata, FileType};
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
    server_updates: &Vec<FileMetadata>,
    last_sync: u64,
) -> Result<WorkCalculated, CoreError> {
    let mut most_recent_update_from_server: u64 = last_sync;
    let mut work_units: Vec<WorkUnit> = vec![];
    for metadata in server_updates {
        if metadata.metadata_version > most_recent_update_from_server {
            most_recent_update_from_server = metadata.metadata_version;
        }

        match file_repo::maybe_get_metadata(config, RepoSource::Local, metadata.id)? {
            None => {
                if !metadata.deleted {
                    // no work for files we don't have that have been deleted
                    work_units.push(WorkUnit::ServerChange {
                        metadata: metadata.clone(),
                    })
                }
            }
            Some(local_metadata) => {
                if metadata.metadata_version != local_metadata.metadata_version {
                    work_units.push(WorkUnit::ServerChange {
                        metadata: metadata.clone(),
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

fn merge_metadata(base: FileMetadata, local: FileMetadata, remote: FileMetadata) -> FileMetadata {
    let local_renamed = local.name.hmac != base.name.hmac;
    let remote_renamed = remote.name.hmac != base.name.hmac;
    let name = match (local_renamed, remote_renamed) {
        (false, false) => base.name,
        (true, false) => local.name,
        (false, true) => remote.name,
        (true, true) => remote.name, // resolve rename conflicts in favor of remote
    };

    let local_moved = local.parent != base.parent;
    let remote_moved = remote.parent != remote.parent;
    let (parent, folder_access_keys) = match (local_moved, remote_moved) {
        (false, false) => (base.parent, base.folder_access_keys),
        (true, false) => (local.parent, local.folder_access_keys),
        (false, true) => (remote.parent, remote.folder_access_keys),
        (true, true) => (remote.parent, remote.folder_access_keys), // resolve move conflicts in favor of remote
    };

    FileMetadata {
        id: base.id,               // ids never change
        file_type: base.file_type, // file types never change
        parent,
        name,
        owner: base.owner,                         // owners never change
        metadata_version: remote.metadata_version, // resolve metadata version conflicts in favor of remote
        content_version: remote.content_version, // resolve content version conflicts in favor of remote
        deleted: base.deleted || local.deleted || remote.deleted, // resolve delete conflicts by deleting
        user_access_keys: base.user_access_keys,                  // user access keys never change
        folder_access_keys,
    }
}


// sync write: submit changes to remote, validate on local (errors -> resolve)
// sync read: get all local changes, send to server (both can be metadata_repo or doc_repo)
// todo: this function is too long!

pub fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), CoreError> {
    let account = &account_repo::get(config)?;

    // pull remote changes
    let last_sync = last_updated_repo::get(config)?;
    let remote_changes = client::request(
        account,
        GetUpdatesRequest {
            since_metadata_version: last_sync,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;
    let total = calculate_work_from_updates(config, &remote_changes, last_sync)?
        .work_units
        .len();
    let mut progress = 0;

    // merge with local changes; save results locally
    for remote_metadata in remote_changes {
        if let Some(ref func) = f {
            func(SyncProgress {
                total,
                progress,
                current_work_unit: generate_client_work_unit(
                    config,
                    &WorkUnit::ServerChange {
                        metadata: remote_metadata.clone(),
                    },
                )?,
            })
        }

        let maybe_base_metadata =
            file_service::maybe_get_metadata(config, RepoSource::Remote, remote_metadata.id)?
                .map(|(m, _)| m);
        let maybe_local_metadata =
            file_service::maybe_get_metadata(config, RepoSource::Local, remote_metadata.id)?
                .map(|(m, _)| m);

        // merge metadata
        let merged_metadata = match maybe_merge(
            maybe_base_metadata.clone(),
            maybe_local_metadata,
            Some(remote_metadata.clone()),
        )? {
            MaybeMergeResult::Resolved(merged) => merged,
            MaybeMergeResult::Conflict {
                base,
                local,
                remote,
            } => merge_metadata(base, local, remote),
        };

        // merge document content
        if remote_metadata.file_type == FileType::Document {
            let content_updated = if let Some(base) = maybe_base_metadata {
                remote_metadata.content_version != base.content_version
            } else {
                true
            };
            if content_updated {
                let remote_document = file_service::read_document_content(
                    config,
                    &remote_metadata,
                    &client::request(
                        &account,
                        GetDocumentRequest {
                            id: remote_metadata.id,
                            content_version: remote_metadata.content_version,
                        },
                    )?
                    .content,
                )?;

                let maybe_base_document = file_service::maybe_read_document(
                    config,
                    RepoSource::Remote,
                    remote_metadata.id,
                )?;
                let maybe_local_document = file_service::maybe_read_document(
                    config,
                    RepoSource::Local,
                    remote_metadata.id,
                )?;

                // update remote repo to version from server
                file_service::write_document(
                    config,
                    RepoSource::Remote,
                    remote_metadata.id,
                    &remote_document,
                )?;

                // merge document content for documents with updated content
                let merged_document = match maybe_merge(
                    maybe_base_document,
                    maybe_local_document,
                    Some(remote_document),
                )? {
                    MaybeMergeResult::Resolved(merged_document) => merged_document,
                    MaybeMergeResult::Conflict {
                        base: base_document,
                        local: local_document,
                        remote: remote_document,
                    } => {
                        match DocumentType::from_file_name_using_extension(
                            &file_encryption_service::get_name(config, &remote_metadata)?,
                        ) {
                            // text documents get 3-way merged
                            DocumentType::Text => {
                                match diffy::merge_bytes(
                                    &base_document,
                                    &local_document,
                                    &remote_document,
                                ) {
                                    Ok(without_conflicts) => without_conflicts,
                                    Err(with_conflicts) => with_conflicts,
                                }
                            }
                            // other documents have local version copied to new file
                            DocumentType::Drawing | DocumentType::Other => {
                                let remote_name =
                                    file_encryption_service::get_name(config, &remote_metadata)?;
                                file_service::create(
                                    config,
                                    RepoSource::Local,
                                    &format!(
                                        "{}-CONTENT-CONFLICT-{}",
                                        &remote_name,
                                        remote_metadata.id.clone()
                                    ),
                                    remote_metadata.parent.clone(),
                                    FileType::Document,
                                )?;

                                file_service::write_document(
                                    config,
                                    RepoSource::Local,
                                    remote_metadata.id.clone(),
                                    &file_service::read_document(
                                        config,
                                        RepoSource::Local,
                                        remote_metadata.id,
                                    )?,
                                )?;

                                remote_document
                            }
                        }
                    }
                };

                // update local repo to version from merge
                file_service::write_document(
                    config,
                    RepoSource::Local,
                    remote_metadata.id,
                    &merged_document,
                )?;
            }
        }

        // update remote repo to version from server
        file_service::insert_metadata(config, RepoSource::Remote, &remote_metadata)?;

        // resolve path conflicts
        if file_repo::get_children(config, RepoSource::Local, merged_metadata.parent)?
            .into_iter()
            .any(|f| f.id != merged_metadata.id && f.name.hmac == merged_metadata.name.hmac)
        {
            file_service::rename(
                config,
                RepoSource::Local,
                merged_metadata.id,
                &format!(
                    "{}-NAME-CONFLICT-{}",
                    file_encryption_service::get_name(&config, &merged_metadata)?,
                    merged_metadata.id
                ),
            )?
        }

        // update local repo to version from merge
        file_repo::insert_metadata(config, RepoSource::Local, &merged_metadata)?;

        // finished remote work unit
        progress += 1;
    }

    // push local content changes
    for id in file_repo::get_all_with_document_changes(config)? {
        let local_metadata = file_repo::get_metadata(config, RepoSource::Local, id)?;

        if let Some(ref func) = f {
            func(SyncProgress {
                total,
                progress,
                current_work_unit: generate_client_work_unit(
                    config,
                    &WorkUnit::ServerChange {
                        metadata: local_metadata.clone(),
                    },
                )?,
            })
        }

        client::request(
            &account,
            ChangeDocumentContentRequest {
                id: id,
                old_metadata_version: local_metadata.metadata_version,
                new_content: file_repo::get_document(config, RepoSource::Local, id)?,
            },
        )
        .map_err(CoreError::from)?;

        // finished local work unit
        progress += 1;
    }

    // push local metadata changes
    client::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: file_repo::get_all_metadata_changes(config)?,
        },
    )
    .map_err(CoreError::from)?;

    file_repo::prune_deleted(config)?;

    Ok(())
}
