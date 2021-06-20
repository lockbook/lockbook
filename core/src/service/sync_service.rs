use crate::model::client_conversion::{generate_client_work_unit, ClientWorkUnit};
use crate::model::state::Config;
use crate::repo::{account_repo, document_repo, file_metadata_repo, local_changes_repo};
use crate::service::file_compression_service;
use crate::service::{file_encryption_service, file_service};
use crate::{client, CoreError};
use lockbook_models::account::Account;
use lockbook_models::api::{
    ChangeDocumentContentRequest, CreateDocumentRequest, CreateFolderRequest,
    DeleteDocumentRequest, DeleteFolderRequest, GetDocumentRequest, GetUpdatesRequest,
    MoveDocumentRequest, MoveFolderRequest, RenameDocumentRequest, RenameFolderRequest,
};
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::local_changes::{Edited, LocalChange as LocalChangeRepoLocalChange};
use lockbook_models::work_unit::WorkUnit;
use lockbook_models::work_unit::WorkUnit::{LocalChange, ServerChange};
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

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, CoreError> {
    info!("Calculating Work");
    let mut work_units: Vec<WorkUnit> = vec![];

    let account = account_repo::get_account(config)?;
    let last_sync = file_metadata_repo::get_last_updated(config)?;

    let server_updates = client::request(
        &account,
        GetUpdatesRequest {
            since_metadata_version: last_sync,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    let mut most_recent_update_from_server: u64 = last_sync;
    for metadata in server_updates {
        if metadata.metadata_version > most_recent_update_from_server {
            most_recent_update_from_server = metadata.metadata_version;
        }

        match file_metadata_repo::maybe_get(config, metadata.id)? {
            None => {
                if !metadata.deleted {
                    // no work for files we don't have that have been deleted
                    work_units.push(ServerChange { metadata })
                }
            }
            Some(local_metadata) => {
                if metadata.metadata_version != local_metadata.metadata_version {
                    work_units.push(ServerChange { metadata })
                }
            }
        };
    }

    work_units.sort_by(|f1, f2| {
        f1.get_metadata()
            .metadata_version
            .cmp(&f2.get_metadata().metadata_version)
    });

    let changes = local_changes_repo::get_all_local_changes(config)?;

    for change_description in changes {
        let metadata = file_metadata_repo::get(config, change_description.id)?;

        work_units.push(LocalChange { metadata });
    }
    debug!("Work Calculated: {:#?}", work_units);

    Ok(WorkCalculated {
        work_units,
        most_recent_update_from_server,
    })
}

pub fn execute_work(config: &Config, account: &Account, work: WorkUnit) -> Result<(), CoreError> {
    match work {
        WorkUnit::LocalChange { mut metadata } => {
            handle_local_change(config, &account, &mut metadata)
        }
        WorkUnit::ServerChange { mut metadata } => {
            handle_server_change(config, &account, &mut metadata)
        }
    }
}

pub fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), CoreError> {
    let account = account_repo::get_account(config)?;
    let mut sync_errors: HashMap<Uuid, CoreError> = HashMap::new();
    let mut work_calculated = calculate_work(config)?;

    // Retry sync n times
    for _ in 0..10 {
        info!("Syncing");

        for (progress, work_unit) in work_calculated.work_units.iter().enumerate() {
            if let Some(ref func) = f {
                func(SyncProgress {
                    total: work_calculated.work_units.len(),
                    progress,
                    current_work_unit: generate_client_work_unit(config, &work_unit)?,
                })
            }

            match execute_work(config, &account, work_unit.clone()) {
                Ok(_) => {
                    debug!("{:#?} executed successfully", work_unit);
                    sync_errors.remove(&work_unit.get_metadata().id);
                }
                Err(err) => {
                    error!("Sync error detected: {:#?} {:#?}", work_unit, err);
                    sync_errors.insert(work_unit.get_metadata().id, err);
                }
            }
        }

        if sync_errors.is_empty() {
            file_metadata_repo::set_last_synced(
                config,
                work_calculated.most_recent_update_from_server,
            )?;
        }

        work_calculated = calculate_work(config)?;
        if work_calculated.work_units.is_empty() {
            break;
        }
    }

    if sync_errors.is_empty() {
        file_metadata_repo::set_last_synced(
            config,
            work_calculated.most_recent_update_from_server,
        )?;
        Ok(())
    } else {
        error!("We finished everything calculate work told us about, but still have errors, this is concerning, the errors are: {:#?}", sync_errors);
        Err(CoreError::Unexpected(format!(
            "work execution errors: {:#?}",
            sync_errors
        )))
    }
}

/// Paths within lockbook must be unique. Prior to handling a server change we make sure that
/// there are not going to be path conflicts. If there are, we find the file that is conflicting
/// locally and rename it
fn rename_local_conflicting_files(
    config: &Config,
    metadata: &FileMetadata,
) -> Result<(), CoreError> {
    let conflicting_files =
        file_metadata_repo::get_children_non_recursively(config, metadata.parent)?
            .into_iter()
            .filter(|potential_conflict| potential_conflict.name == metadata.name)
            .filter(|potential_conflict| potential_conflict.id != metadata.id)
            .collect::<Vec<FileMetadata>>();

    // There should only be one of these
    for conflicting_file in conflicting_files {
        let old_name = file_encryption_service::get_name(&config, &conflicting_file)?;
        file_service::rename_file(
            config,
            conflicting_file.id,
            &format!("{}-NAME-CONFLICT-{}", old_name, conflicting_file.id),
        )?
    }

    Ok(())
}

/// Save metadata locally, and download the file contents if this file is a Document
fn save_file_locally(
    config: &Config,
    account: &Account,
    metadata: &FileMetadata,
) -> Result<(), CoreError> {
    file_metadata_repo::insert(config, &metadata)?;

    if metadata.file_type == Document {
        let document = client::request(
            &account,
            GetDocumentRequest {
                id: metadata.id,
                content_version: metadata.content_version,
            },
        )
        .map_err(CoreError::from)?
        .content;

        document_repo::insert(config, metadata.id, &document)?;
    }

    Ok(())
}

fn delete_file_locally(config: &Config, metadata: &FileMetadata) -> Result<(), CoreError> {
    if metadata.file_type == Document {
        // A deleted document
        file_metadata_repo::non_recursive_delete(config, metadata.id)?;
        local_changes_repo::delete(config, metadata.id)?;
        document_repo::delete(config, metadata.id)?
    } else {
        // A deleted folder
        let delete_errors =
            file_metadata_repo::get_and_get_children_recursively(config, metadata.id)?
                .into_iter()
                .map(|file_metadata| -> Option<String> {
                    match document_repo::delete(config, file_metadata.id) {
                        Ok(_) => {
                            match file_metadata_repo::non_recursive_delete(config, metadata.id) {
                                Ok(_) => match local_changes_repo::delete(config, metadata.id) {
                                    Ok(_) => None,
                                    Err(err) => Some(format!("{:?}", err)),
                                },
                                Err(err) => Some(format!("{:?}", err)),
                            }
                        }
                        Err(err) => Some(format!("{:?}", err)),
                    }
                })
                .flatten()
                .collect::<Vec<String>>();

        if !delete_errors.is_empty() {
            return Err(CoreError::Unexpected(format!(
                "delete errors: {:#?}",
                delete_errors
            )));
        }
    }

    Ok(())
}

fn merge_documents(
    config: &Config,
    account: &Account,
    metadata: &mut FileMetadata,
    local_metadata: &FileMetadata,
    local_changes: &LocalChangeRepoLocalChange,
    edited_locally: &Edited,
) -> Result<(), CoreError> {
    let local_name = file_encryption_service::get_name(&config, &local_metadata)?;
    if local_name.ends_with(".md") || local_name.ends_with(".txt") {
        let common_ancestor = {
            let compressed_common_ancestor = file_encryption_service::user_read_document(
                &account,
                &edited_locally.old_value,
                &edited_locally.access_info,
            )?;

            file_compression_service::decompress(&compressed_common_ancestor)?
        };

        let current_version = file_service::read_document(config, metadata.id)?;

        let server_version = {
            let server_document = client::request(
                &account,
                GetDocumentRequest {
                    id: metadata.id,
                    content_version: metadata.content_version,
                },
            )?
            .content;

            let compressed_server_version = file_encryption_service::user_read_document(
                &account,
                &server_document,
                &edited_locally.access_info,
            )?;
            // This assumes that a file is never re-keyed.

            file_compression_service::decompress(&compressed_server_version)?
        };

        let result = match diffy::merge_bytes(&common_ancestor, &current_version, &server_version) {
            Ok(no_conflicts) => no_conflicts,
            Err(conflicts) => conflicts,
        };

        file_service::write_document(config, metadata.id, &result)?;
    } else {
        // Create a new file
        let new_file = file_service::create(
            config,
            &format!("{}-CONTENT-CONFLICT-{}", &local_name, local_metadata.id),
            local_metadata.parent,
            Document,
        )?;

        // Copy the local copy over
        document_repo::insert(
            config,
            new_file.id,
            &document_repo::get(config, local_changes.id)?,
        )?;

        // Overwrite local file with server copy
        let new_content = client::request(
            &account,
            GetDocumentRequest {
                id: metadata.id,
                content_version: metadata.content_version,
            },
        )
        .map_err(CoreError::from)?
        .content;

        document_repo::insert(config, metadata.id, &new_content)?;

        // Mark content as synced
        local_changes_repo::untrack_edit(config, metadata.id)?;
    }

    Ok(())
}

fn merge_files(
    config: &Config,
    account: &Account,
    metadata: &mut FileMetadata,
    local_metadata: &FileMetadata,
    local_changes: &LocalChangeRepoLocalChange,
) -> Result<(), CoreError> {
    if let Some(renamed_locally) = &local_changes.renamed {
        // Check if both renamed, if so, server wins
        let server_name = file_encryption_service::get_name(&config, &metadata)?;
        if server_name != renamed_locally.old_value {
            local_changes_repo::untrack_rename(config, metadata.id)?;
        } else {
            metadata.name = local_metadata.name.clone();
        }
    }

    // We moved it locally
    if let Some(moved_locally) = &local_changes.moved {
        // Check if both moved, if so server wins
        if metadata.parent != moved_locally.old_value {
            local_changes_repo::untrack_move(config, metadata.id)?;
        } else {
            metadata.parent = local_metadata.parent;
            metadata.folder_access_keys = local_metadata.folder_access_keys.clone();
        }
    }

    if let Some(edited_locally) = &local_changes.content_edited {
        info!("Content conflict for: {}", metadata.id);
        if local_metadata.content_version != metadata.content_version {
            if metadata.file_type == Folder {
                // Should be unreachable
                error!("Not only was a folder edited, it was edited according to the server as well. This should not be possible, id: {}", metadata.id);
            }

            merge_documents(
                &config,
                &account,
                metadata,
                &local_metadata,
                &local_changes,
                edited_locally,
            )?;
        }
    }

    Ok(())
}

fn handle_server_change(
    config: &Config,
    account: &Account,
    metadata: &mut FileMetadata,
) -> Result<(), CoreError> {
    rename_local_conflicting_files(&config, &metadata)?;

    match file_metadata_repo::maybe_get(config, metadata.id)? {
        None => {
            if !metadata.deleted {
                save_file_locally(&config, &account, &metadata)?;
            } else {
                debug!(
                    "Server deleted a file we don't know about, ignored. id: {:?}",
                    metadata.id
                );
            }
        }
        Some(local_metadata) => {
            match local_changes_repo::get_local_changes(config, metadata.id)? {
                None => {
                    if metadata.deleted {
                        delete_file_locally(&config, &metadata)?;
                    } else {
                        save_file_locally(&config, &account, &metadata)?;
                    }
                }
                Some(local_changes) => {
                    if !local_changes.deleted && !metadata.deleted {
                        merge_files(&config, &account, metadata, &local_metadata, &local_changes)?;

                        file_metadata_repo::insert(config, &metadata)?;
                    } else if metadata.deleted {
                        // Adding checks here is how you can protect local state from being deleted
                        delete_file_locally(&config, &metadata)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn handle_local_change(
    config: &Config,
    account: &Account,
    metadata: &mut FileMetadata,
) -> Result<(), CoreError> {
    match local_changes_repo::get_local_changes(config, metadata.id)? {
                None => debug!("Calculate work indicated there was work to be done, but local_changes_repo didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
                Some(mut local_change) => {
                    if local_change.new {
                        if metadata.file_type == Document {
                            let content = document_repo::get(config, metadata.id)?;
                            let version = client::request(
                                &account,
                                CreateDocumentRequest::new(&metadata, content),
                            )
                                .map_err(CoreError::from)?
                                .new_metadata_and_content_version;

                            metadata.metadata_version = version;
                            metadata.content_version = version;
                        } else {
                            let version = client::request(
                                &account,
                                CreateFolderRequest::new(&metadata),
                            )
                                .map_err(CoreError::from)?
                                .new_metadata_version;

                            metadata.metadata_version = version;
                            metadata.content_version = version;
                        }

                        file_metadata_repo::insert(config, &metadata)?;

                        local_changes_repo::untrack_new_file(config, metadata.id)?;
                        local_change.new = false;
                        local_change.renamed = None;
                        local_change.content_edited = None;
                        local_change.moved = None;

                        // return early to allow any other child operations like move can be sent to the
                        // server
                        if local_change.deleted && metadata.file_type == Folder {
                            return Ok(());
                        }
                    }

                    if local_change.renamed.is_some() {
                        let version = if metadata.file_type == Document {
                            client::request(&account, RenameDocumentRequest::new(&metadata))
                                .map_err(CoreError::from)?.new_metadata_version
                        } else {
                            client::request(&account, RenameFolderRequest::new(&metadata))
                                .map_err(CoreError::from)?.new_metadata_version
                        };
                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata)?;

                        local_changes_repo::untrack_rename(config, metadata.id)?;
                        local_change.renamed = None;
                    }

                    if local_change.moved.is_some() {
                        metadata.metadata_version = if metadata.file_type == Document {
                            client::request(&account, RenameDocumentRequest::new(&metadata))
                                .map_err(CoreError::from)?.new_metadata_version
                        } else {
                            client::request(&account, RenameFolderRequest::new(&metadata))
                                .map_err(CoreError::from)?.new_metadata_version
                        };

                        let version = if metadata.file_type == Document {
                            client::request(&account, MoveDocumentRequest::new(&metadata)).map_err(CoreError::from)?.new_metadata_version
                        } else {
                            client::request(&account, MoveFolderRequest::new(&metadata)).map_err(CoreError::from)?.new_metadata_version
                        };

                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata)?;

                        local_changes_repo::untrack_move(config, metadata.id)?;
                        local_change.moved = None;
                    }

                    if local_change.content_edited.is_some() && metadata.file_type == Document {
                        let version = client::request(&account, ChangeDocumentContentRequest{
                            id: metadata.id,
                            old_metadata_version: metadata.metadata_version,
                            new_content: document_repo::get(config, metadata.id)?,
                        }).map_err(CoreError::from)?.new_metadata_and_content_version;

                        metadata.content_version = version;
                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata)?;

                        local_changes_repo::untrack_edit(config, metadata.id)?;
                        local_change.content_edited = None;
                    }

                    if local_change.deleted {
                        if metadata.file_type == Document {
                            client::request(&account, DeleteDocumentRequest{ id: metadata.id }).map_err(CoreError::from)?;
                        } else {
                            client::request(&account, DeleteFolderRequest{ id: metadata.id }).map_err(CoreError::from)?;
                        }

                        local_changes_repo::delete(config, metadata.id)?;
                        local_change.deleted = false;

                        file_metadata_repo::non_recursive_delete(config, metadata.id)?; // Now it's safe to delete this locally
                    }
                }
            }
    Ok(())
}
