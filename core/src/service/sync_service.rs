use std::collections::HashMap;
use std::time::SystemTimeError;

use serde::Serialize;
use uuid::Uuid;

use crate::client;
use crate::client::ApiError;
use crate::model::state::Config;
use crate::repo::{account_repo, document_repo, file_metadata_repo, local_changes_repo};
use crate::service::file_compression_service::FileCompressionService;
use crate::service::file_service::{FileService, NewFileFromPathError};
use crate::service::sync_service::CalculateWorkError::{
    AccountRetrievalError, GetMetadataError, GetUpdatesError, LocalChangesRepoError,
    MetadataRepoError,
};
use crate::service::sync_service::WorkExecutionError::{
    AutoRenameError, DecompressingForMergeError, DecryptingOldVersionForMergeError,
    ReadingCurrentVersionError, RecursiveDeleteError, ResolveConflictByCreatingNewFileError,
    SaveDocumentError, WritingMergedFileError,
};
use crate::service::{file_encryption_service, file_service};
use lockbook_crypto::pubkey::ECSignError;
use lockbook_models::account::Account;
use lockbook_models::api;
use lockbook_models::api::{
    ChangeDocumentContentError, ChangeDocumentContentRequest, CreateDocumentError,
    CreateDocumentRequest, CreateFolderError, CreateFolderRequest, DeleteDocumentError,
    DeleteDocumentRequest, DeleteFolderError, DeleteFolderRequest, GetDocumentError,
    GetDocumentRequest, GetUpdatesRequest, MoveDocumentError, MoveDocumentRequest, MoveFolderError,
    MoveFolderRequest, RenameDocumentError, RenameDocumentRequest, RenameFolderError,
    RenameFolderRequest,
};
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::local_changes::{Edited, LocalChange as LocalChangeRepoLocalChange};
use lockbook_models::work_unit::WorkUnit;
use lockbook_models::work_unit::WorkUnit::{LocalChange, ServerChange};

#[derive(Debug)]
pub enum CalculateWorkError {
    LocalChangesRepoError(local_changes_repo::DbError),
    MetadataRepoError(file_metadata_repo::GetError),
    GetMetadataError(file_metadata_repo::DbError),
    AccountRetrievalError(account_repo::AccountRepoError),
    GetUpdatesError(client::ApiError<api::GetUpdatesError>),
}

// TODO standardize enum variant notation within core
#[derive(Debug)]
pub enum WorkExecutionError {
    MetadataRepoError(file_metadata_repo::DbError),
    MetadataRepoErrorOpt(file_metadata_repo::GetError),
    DocumentGetError(GetDocumentError),
    DocumentRenameError(RenameDocumentError),
    FolderRenameError(RenameFolderError),
    DocumentMoveError(MoveDocumentError),
    FolderMoveError(MoveFolderError),
    DocumentCreateError(CreateDocumentError),
    FolderCreateError(CreateFolderError),
    DocumentChangeError(ChangeDocumentContentError),
    DocumentDeleteError(DeleteDocumentError),
    FolderDeleteError(DeleteFolderError),
    RecursiveDeleteError(Vec<String>),
    LocalFolderDeleteError(file_service::DeleteFolderError),
    FindingChildrenFailed(file_metadata_repo::FindingChildrenFailed),
    SaveDocumentError(document_repo::Error),
    // Delete uses this and it shouldn't
    // TODO make more general
    LocalChangesRepoError(local_changes_repo::DbError),
    AutoRenameError(file_service::DocumentRenameError),
    ResolveConflictByCreatingNewFileError(file_service::NewFileError),
    DecryptingOldVersionForMergeError(file_encryption_service::UnableToReadFileAsUser),
    DecompressingForMergeError(std::io::Error),
    ReadingCurrentVersionError(file_service::ReadDocumentError),
    WritingMergedFileError(file_service::DocumentUpdateError),
    FindingParentsForConflictingFileError(file_metadata_repo::FindingParentsFailed),
    ErrorCreatingRecoveryFile(NewFileFromPathError),
    ErrorCalculatingCurrentTime(SystemTimeError),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(ECSignError),
    Serialize(serde_json::error::Error),
    SendFailed(reqwest::Error),
    ReceiveFailed(reqwest::Error),
    Deserialize(serde_json::error::Error),
}

#[derive(Debug)]
pub enum SyncError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CalculateWorkError(CalculateWorkError),
    WorkExecutionError(HashMap<Uuid, WorkExecutionError>),
    MetadataUpdateError(file_metadata_repo::DbError),
}

pub trait SyncService {
    fn calculate_work(config: &Config) -> Result<WorkCalculated, CalculateWorkError>;
    fn execute_work(
        config: &Config,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError>;
    fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), SyncError>;
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: WorkUnit,
}

pub struct FileSyncService<Files: FileService, FileCompression: FileCompressionService> {
    _file: Files,
    _file_compression: FileCompression,
}

impl<Files: FileService, FileCompression: FileCompressionService> SyncService
    for FileSyncService<Files, FileCompression>
{
    fn calculate_work(config: &Config) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating Work");
        let mut work_units: Vec<WorkUnit> = vec![];

        let account = account_repo::get_account(config).map_err(AccountRetrievalError)?;
        let last_sync = file_metadata_repo::get_last_updated(config).map_err(GetMetadataError)?;

        let server_updates = client::request(
            &account,
            GetUpdatesRequest {
                since_metadata_version: last_sync,
            },
        )
        .map_err(GetUpdatesError)?
        .file_metadata;

        let mut most_recent_update_from_server: u64 = last_sync;
        for metadata in server_updates {
            if metadata.metadata_version > most_recent_update_from_server {
                most_recent_update_from_server = metadata.metadata_version;
            }

            match file_metadata_repo::maybe_get(config, metadata.id).map_err(GetMetadataError)? {
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

        let changes =
            local_changes_repo::get_all_local_changes(config).map_err(LocalChangesRepoError)?;

        for change_description in changes {
            let metadata = file_metadata_repo::get(config, change_description.id)
                .map_err(MetadataRepoError)?;

            work_units.push(LocalChange { metadata });
        }
        debug!("Work Calculated: {:#?}", work_units);

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }
    fn execute_work(
        config: &Config,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError> {
        match work {
            WorkUnit::LocalChange { mut metadata } => {
                Self::handle_local_change(config, &account, &mut metadata)
            }
            WorkUnit::ServerChange { mut metadata } => {
                Self::handle_server_change(config, &account, &mut metadata)
            }
        }
    }

    fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), SyncError> {
        let account =
            account_repo::get_account(config).map_err(SyncError::AccountRetrievalError)?;

        let mut sync_errors: HashMap<Uuid, WorkExecutionError> = HashMap::new();

        let mut work_calculated =
            Self::calculate_work(config).map_err(SyncError::CalculateWorkError)?;

        // Retry sync n times
        for _ in 0..10 {
            info!("Syncing");

            for (progress, work_unit) in work_calculated.work_units.iter().enumerate() {
                if let Some(ref func) = f {
                    func(SyncProgress {
                        total: work_calculated.work_units.len(),
                        progress,
                        current_work_unit: work_unit.clone(),
                    })
                }

                match Self::execute_work(config, &account, work_unit.clone()) {
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
                )
                .map_err(SyncError::MetadataUpdateError)?;
            }

            work_calculated =
                Self::calculate_work(config).map_err(SyncError::CalculateWorkError)?;

            if work_calculated.work_units.is_empty() {
                break;
            }
        }

        if sync_errors.is_empty() {
            file_metadata_repo::set_last_synced(
                config,
                work_calculated.most_recent_update_from_server,
            )
            .map_err(SyncError::MetadataUpdateError)?;
            Ok(())
        } else {
            error!("We finished everything calculate work told us about, but still have errors, this is concerning, the errors are: {:#?}", sync_errors);
            Err(SyncError::WorkExecutionError(sync_errors))
        }
    }
}

/// Helper functions
impl<
        Files: FileService,
        FileCompression: FileCompressionService,
    > FileSyncService<Files, FileCompression>
{
    /// Paths within lockbook must be unique. Prior to handling a server change we make sure that
    /// there are not going to be path conflicts. If there are, we find the file that is conflicting
    /// locally and rename it
    fn rename_local_conflicting_files(
        config: &Config,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        let conflicting_files =
            file_metadata_repo::get_children_non_recursively(config, metadata.parent)
                .map_err(WorkExecutionError::MetadataRepoError)?
                .into_iter()
                .filter(|potential_conflict| potential_conflict.name == metadata.name)
                .filter(|potential_conflict| potential_conflict.id != metadata.id)
                .collect::<Vec<FileMetadata>>();

        // There should only be one of these
        for conflicting_file in conflicting_files {
            Files::rename_file(
                config,
                conflicting_file.id,
                &format!(
                    "{}-NAME-CONFLICT-{}",
                    conflicting_file.name, conflicting_file.id
                ),
            )
            .map_err(AutoRenameError)?
        }

        Ok(())
    }

    /// Save metadata locally, and download the file contents if this file is a Document
    fn save_file_locally(
        config: &Config,
        account: &Account,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        file_metadata_repo::insert(config, &metadata)
            .map_err(WorkExecutionError::MetadataRepoError)?;

        if metadata.file_type == Document {
            let document = client::request(
                &account,
                GetDocumentRequest {
                    id: metadata.id,
                    content_version: metadata.content_version,
                },
            )
            .map_err(WorkExecutionError::from)?
            .content;

            document_repo::insert(config, metadata.id, &document).map_err(SaveDocumentError)?;
        }

        Ok(())
    }

    fn delete_file_locally(
        config: &Config,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        if metadata.file_type == Document {
            // A deleted document
            file_metadata_repo::non_recursive_delete(config, metadata.id)
                .map_err(WorkExecutionError::MetadataRepoError)?;

            local_changes_repo::delete(config, metadata.id)
                .map_err(WorkExecutionError::LocalChangesRepoError)?;

            document_repo::delete(config, metadata.id).map_err(SaveDocumentError)?
        } else {
            // A deleted folder
            let delete_errors =
                file_metadata_repo::get_and_get_children_recursively(config, metadata.id)
                    .map_err(WorkExecutionError::FindingChildrenFailed)?
                    .into_iter()
                    .map(|file_metadata| -> Option<String> {
                        match document_repo::delete(config, file_metadata.id) {
                            Ok(_) => {
                                match file_metadata_repo::non_recursive_delete(config, metadata.id)
                                {
                                    Ok(_) => {
                                        match local_changes_repo::delete(config, metadata.id) {
                                            Ok(_) => None,
                                            Err(err) => Some(format!("{:?}", err)),
                                        }
                                    }
                                    Err(err) => Some(format!("{:?}", err)),
                                }
                            }
                            Err(err) => Some(format!("{:?}", err)),
                        }
                    })
                    .flatten()
                    .collect::<Vec<String>>();

            if !delete_errors.is_empty() {
                return Err(RecursiveDeleteError(delete_errors));
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
    ) -> Result<(), WorkExecutionError> {
        if metadata.name.ends_with(".md") || metadata.name.ends_with(".txt") {
            let common_ancestor = {
                let compressed_common_ancestor = file_encryption_service::user_read_document(
                    &account,
                    &edited_locally.old_value,
                    &edited_locally.access_info,
                )
                .map_err(DecryptingOldVersionForMergeError)?;

                FileCompression::decompress(&compressed_common_ancestor)
                    .map_err(DecompressingForMergeError)?
            };

            let current_version =
                Files::read_document(config, metadata.id).map_err(ReadingCurrentVersionError)?;

            let server_version = {
                let server_document = client::request(
                    &account,
                    GetDocumentRequest {
                        id: metadata.id,
                        content_version: metadata.content_version,
                    },
                )
                .map_err(WorkExecutionError::from)?
                .content;

                let compressed_server_version = file_encryption_service::user_read_document(
                    &account,
                    &server_document,
                    &edited_locally.access_info,
                )
                .map_err(DecryptingOldVersionForMergeError)?;
                // This assumes that a file is never re-keyed.

                FileCompression::decompress(&compressed_server_version)
                    .map_err(DecompressingForMergeError)?
            };

            let result =
                match diffy::merge_bytes(&common_ancestor, &current_version, &server_version) {
                    Ok(no_conflicts) => no_conflicts,
                    Err(conflicts) => conflicts,
                };

            Files::write_document(config, metadata.id, &result).map_err(WritingMergedFileError)?;
        } else {
            // Create a new file
            let new_file = Files::create(
                config,
                &format!(
                    "{}-CONTENT-CONFLICT-{}",
                    &local_metadata.name, local_metadata.id
                ),
                local_metadata.parent,
                Document,
            )
            .map_err(ResolveConflictByCreatingNewFileError)?;

            // Copy the local copy over
            document_repo::insert(
                config,
                new_file.id,
                &document_repo::get(config, local_changes.id).map_err(SaveDocumentError)?,
            )
            .map_err(SaveDocumentError)?;

            // Overwrite local file with server copy
            let new_content = client::request(
                &account,
                GetDocumentRequest {
                    id: metadata.id,
                    content_version: metadata.content_version,
                },
            )
            .map_err(WorkExecutionError::from)?
            .content;

            document_repo::insert(config, metadata.id, &new_content).map_err(SaveDocumentError)?;

            // Mark content as synced
            local_changes_repo::untrack_edit(config, metadata.id)
                .map_err(WorkExecutionError::LocalChangesRepoError)?;
        }

        Ok(())
    }

    fn merge_files(
        config: &Config,
        account: &Account,
        metadata: &mut FileMetadata,
        local_metadata: &FileMetadata,
        local_changes: &LocalChangeRepoLocalChange,
    ) -> Result<(), WorkExecutionError> {
        if let Some(renamed_locally) = &local_changes.renamed {
            // Check if both renamed, if so, server wins
            if metadata.name != renamed_locally.old_value {
                local_changes_repo::untrack_rename(config, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
            } else {
                metadata.name = local_metadata.name.clone();
            }
        }

        // We moved it locally
        if let Some(moved_locally) = &local_changes.moved {
            // Check if both moved, if so server wins
            if metadata.parent != moved_locally.old_value {
                local_changes_repo::untrack_move(config, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
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

                Self::merge_documents(
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
    ) -> Result<(), WorkExecutionError> {
        Self::rename_local_conflicting_files(&config, &metadata)?;

        match file_metadata_repo::maybe_get(config, metadata.id)
            .map_err(WorkExecutionError::MetadataRepoError)?
        {
            None => {
                if !metadata.deleted {
                    Self::save_file_locally(&config, &account, &metadata)?;
                } else {
                    debug!(
                        "Server deleted a file we don't know about, ignored. id: {:?}",
                        metadata.id
                    );
                }
            }
            Some(local_metadata) => {
                match local_changes_repo::get_local_changes(config, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?
                {
                    None => {
                        if metadata.deleted {
                            Self::delete_file_locally(&config, &metadata)?;
                        } else {
                            Self::save_file_locally(&config, &account, &metadata)?;
                        }
                    }
                    Some(local_changes) => {
                        if !local_changes.deleted && !metadata.deleted {
                            Self::merge_files(
                                &config,
                                &account,
                                metadata,
                                &local_metadata,
                                &local_changes,
                            )?;

                            file_metadata_repo::insert(config, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                        } else if metadata.deleted {
                            // Adding checks here is how you can protect local state from being deleted
                            Self::delete_file_locally(&config, &metadata)?;
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
    ) -> Result<(), WorkExecutionError> {
        match local_changes_repo::get_local_changes(config, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)? {
                None => debug!("Calculate work indicated there was work to be done, but local_changes_repo didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
                Some(mut local_change) => {
                    if local_change.new {
                        if metadata.file_type == Document {
                            let content = document_repo::get(config, metadata.id).map_err(SaveDocumentError)?;
                            let version = client::request(
                                &account,
                                CreateDocumentRequest::new(&metadata, content),
                            )
                                .map_err(WorkExecutionError::from)?
                                .new_metadata_and_content_version;

                            metadata.metadata_version = version;
                            metadata.content_version = version;
                        } else {
                            let version = client::request(
                                &account,
                                CreateFolderRequest::new(&metadata),
                            )
                                .map_err(WorkExecutionError::from)?
                                .new_metadata_version;

                            metadata.metadata_version = version;
                            metadata.content_version = version;
                        }

                        file_metadata_repo::insert(config, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        local_changes_repo::untrack_new_file(config, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
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
                                .map_err(WorkExecutionError::from)?.new_metadata_version
                        } else {
                            client::request(&account, RenameFolderRequest::new(&metadata))
                                .map_err(WorkExecutionError::from)?.new_metadata_version
                        };
                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        local_changes_repo::untrack_rename(config, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                        local_change.renamed = None;
                    }

                    if local_change.moved.is_some() {
                        let version = if metadata.file_type == Document {
                            client::request(&account, MoveDocumentRequest::new(&metadata)).map_err(WorkExecutionError::from)?.new_metadata_version
                        } else {
                            client::request(&account, MoveFolderRequest::new(&metadata)).map_err(WorkExecutionError::from)?.new_metadata_version
                        };

                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        local_changes_repo::untrack_move(config, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                        local_change.moved = None;
                    }

                    if local_change.content_edited.is_some() && metadata.file_type == Document {
                        let version = client::request(&account, ChangeDocumentContentRequest{
                            id: metadata.id,
                            old_metadata_version: metadata.metadata_version,
                            new_content: document_repo::get(config, metadata.id).map_err(SaveDocumentError)?,
                        }).map_err(WorkExecutionError::from)?.new_metadata_and_content_version;

                        metadata.content_version = version;
                        metadata.metadata_version = version;
                        file_metadata_repo::insert(config, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        local_changes_repo::untrack_edit(config, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                        local_change.content_edited = None;
                    }

                    if local_change.deleted {
                        if metadata.file_type == Document {
                            client::request(&account, DeleteDocumentRequest{ id: metadata.id }).map_err(WorkExecutionError::from)?;
                        } else {
                            client::request(&account, DeleteFolderRequest{ id: metadata.id }).map_err(WorkExecutionError::from)?;
                        }

                        local_changes_repo::delete(config, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                        local_change.deleted = false;

                        file_metadata_repo::non_recursive_delete(config, metadata.id)
                            .map_err(WorkExecutionError::MetadataRepoError)?; // Now it's safe to delete this locally
                    }
                }
            }
        Ok(())
    }
}

fn work_execution_error_from_api_error_common<T>(
    err: ApiError<T>,
) -> Result<WorkExecutionError, T> {
    match err {
        ApiError::Endpoint(e) => Err(e),
        ApiError::ClientUpdateRequired => Ok(WorkExecutionError::ClientUpdateRequired),
        ApiError::InvalidAuth => Ok(WorkExecutionError::InvalidAuth),
        ApiError::ExpiredAuth => Ok(WorkExecutionError::ExpiredAuth),
        ApiError::InternalError => Ok(WorkExecutionError::InternalError),
        ApiError::BadRequest => Ok(WorkExecutionError::BadRequest),
        ApiError::Sign(e) => Ok(WorkExecutionError::Sign(e)),
        ApiError::Serialize(e) => Ok(WorkExecutionError::Serialize(e)),
        ApiError::SendFailed(e) => Ok(WorkExecutionError::SendFailed(e)),
        ApiError::ReceiveFailed(e) => Ok(WorkExecutionError::ReceiveFailed(e)),
        ApiError::Deserialize(e) => Ok(WorkExecutionError::Deserialize(e)),
    }
}

fn ok<T>(r: Result<T, T>) -> T {
    match r {
        Ok(t) => t,
        Err(t) => t,
    }
}

impl From<ApiError<GetDocumentError>> for WorkExecutionError {
    fn from(err: ApiError<GetDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentGetError))
    }
}

impl From<ApiError<RenameDocumentError>> for WorkExecutionError {
    fn from(err: ApiError<RenameDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentRenameError))
    }
}

impl From<ApiError<RenameFolderError>> for WorkExecutionError {
    fn from(err: ApiError<RenameFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderRenameError))
    }
}

impl From<ApiError<MoveDocumentError>> for WorkExecutionError {
    fn from(err: ApiError<MoveDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentMoveError))
    }
}

impl From<ApiError<MoveFolderError>> for WorkExecutionError {
    fn from(err: ApiError<MoveFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderMoveError))
    }
}

impl From<ApiError<CreateDocumentError>> for WorkExecutionError {
    fn from(err: ApiError<CreateDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentCreateError))
    }
}

impl From<ApiError<CreateFolderError>> for WorkExecutionError {
    fn from(err: ApiError<CreateFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderCreateError))
    }
}

impl From<ApiError<ChangeDocumentContentError>> for WorkExecutionError {
    fn from(err: ApiError<ChangeDocumentContentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentChangeError))
    }
}

impl From<ApiError<DeleteDocumentError>> for WorkExecutionError {
    fn from(err: ApiError<DeleteDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentDeleteError))
    }
}

impl From<ApiError<DeleteFolderError>> for WorkExecutionError {
    fn from(err: ApiError<DeleteFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderDeleteError))
    }
}
