use std::collections::HashMap;
use std::time::SystemTimeError;

use serde::Serialize;
use uuid::Uuid;

use crate::client::{ApiError, Client};
use crate::model::account::Account;
use crate::model::api;
use crate::model::api::{
    ChangeDocumentContentError, ChangeDocumentContentRequest, CreateDocumentError,
    CreateDocumentRequest, CreateFolderError, CreateFolderRequest, DeleteDocumentError,
    DeleteDocumentRequest, DeleteFolderError, DeleteFolderRequest, GetDocumentError,
    GetDocumentRequest, GetUpdatesRequest, MoveDocumentError, MoveDocumentRequest, MoveFolderError,
    MoveFolderRequest, RenameDocumentError, RenameDocumentRequest, RenameFolderError,
    RenameFolderRequest,
};
use crate::model::file_metadata::FileMetadata;
use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::work_unit::WorkUnit;
use crate::model::work_unit::WorkUnit::{LocalChange, ServerChange};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, document_repo, file_metadata_repo, local_changes_repo};
use crate::service::crypto_service::RSASignError;
use crate::service::file_compression_service::FileCompressionService;
use crate::service::file_encryption_service::FileEncryptionService;
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
use crate::storage::db_provider::Backend;
use crate::{client, DefaultFileService};

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
    Sign(RSASignError),
    Serialize(serde_json::error::Error),
    SendFailed(reqwest::Error),
    ReceiveFailed(reqwest::Error),
    Deserialize(serde_json::error::Error),
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

#[derive(Debug)]
pub enum SyncError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CalculateWorkError(CalculateWorkError),
    WorkExecutionError(HashMap<Uuid, WorkExecutionError>),
    MetadataUpdateError(file_metadata_repo::DbError),
}

pub trait SyncService {
    fn calculate_work(backend: &Backend) -> Result<WorkCalculated, CalculateWorkError>;
    fn execute_work(
        backend: &Backend,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError>;
    fn sync(backend: &Backend) -> Result<(), SyncError>;
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct FileSyncService<
    FileMetadataDb: FileMetadataRepo,
    ChangeDb: LocalChangesRepo,
    DocsDb: DocumentRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Files: FileService,
    FileCrypto: FileEncryptionService,
    FileCompression: FileCompressionService,
> {
    _metadatas: FileMetadataDb,
    _changes: ChangeDb,
    _docs: DocsDb,
    _accounts: AccountDb,
    _client: ApiClient,
    _file: Files,
    _file_crypto: FileCrypto,
    _file_compression: FileCompression,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        ChangeDb: LocalChangesRepo,
        DocsDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Files: FileService,
        FileCrypto: FileEncryptionService,
        FileCompression: FileCompressionService,
    >
    FileSyncService<
        FileMetadataDb,
        ChangeDb,
        DocsDb,
        AccountDb,
        ApiClient,
        Files,
        FileCrypto,
        FileCompression,
    >
{
    fn handle_server_change(
        backend: &Backend,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        // Make sure no naming conflicts occur as a result of this metadata
        let conflicting_files =
            FileMetadataDb::get_children_non_recursively(backend, metadata.parent)
                .map_err(WorkExecutionError::MetadataRepoError)?
                .into_iter()
                .filter(|potential_conflict| potential_conflict.name == metadata.name)
                .filter(|potential_conflict| potential_conflict.id != metadata.id)
                .collect::<Vec<FileMetadata>>();

        // There should only be one of these
        for conflicting_file in conflicting_files {
            Files::rename_file(
                backend,
                conflicting_file.id,
                &format!(
                    "{}-NAME-CONFLICT-{}",
                    conflicting_file.name, conflicting_file.id
                ),
            )
            .map_err(AutoRenameError)?
        }

        match FileMetadataDb::maybe_get(backend, metadata.id)
            .map_err(WorkExecutionError::MetadataRepoError)?
        {
            None => {
                if !metadata.deleted {
                    // We don't know anything about this file, just do a pull
                    FileMetadataDb::insert(backend, &metadata)
                        .map_err(WorkExecutionError::MetadataRepoError)?;
                    if metadata.file_type == Document {
                        let document = ApiClient::request(
                            &account,
                            GetDocumentRequest {
                                id: metadata.id,
                                content_version: metadata.content_version,
                            },
                        )
                        .map_err(WorkExecutionError::from)?
                        .content;

                        DocsDb::insert(backend, metadata.id, &document)
                            .map_err(SaveDocumentError)?;
                    }
                } else {
                    debug!(
                        "Server deleted a file we don't know about, ignored. id: {:?}",
                        metadata.id
                    );
                }
            }
            Some(local_metadata) => {
                // We have this file locally
                match ChangeDb::get_local_changes(backend, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?
                {
                    None => {
                        // It has no modifications of any sort, just update it
                        if metadata.deleted {
                            if metadata.file_type == Document {
                                // A deleted document
                                FileMetadataDb::non_recursive_delete(backend, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoError)?;

                                ChangeDb::delete(backend, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;

                                DocsDb::delete(backend, metadata.id).map_err(SaveDocumentError)?
                            } else {
                                // A deleted folder
                                let delete_errors =
                                    FileMetadataDb::get_and_get_children_recursively(
                                        backend,
                                        metadata.id,
                                    )
                                    .map_err(WorkExecutionError::FindingChildrenFailed)?
                                    .into_iter()
                                    .map(|file_metadata| -> Option<String> {
                                        match DocsDb::delete(backend, file_metadata.id) {
                                            Ok(_) => {
                                                match FileMetadataDb::non_recursive_delete(
                                                    backend,
                                                    metadata.id,
                                                ) {
                                                    Ok(_) => {
                                                        match ChangeDb::delete(backend, metadata.id)
                                                        {
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
                        } else {
                            // The normal fast forward case
                            FileMetadataDb::insert(backend, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                            if metadata.file_type == Document
                                && local_metadata.metadata_version != metadata.metadata_version
                            {
                                let document = ApiClient::request(
                                    &account,
                                    GetDocumentRequest {
                                        id: metadata.id,
                                        content_version: metadata.content_version,
                                    },
                                )
                                .map_err(WorkExecutionError::from)?
                                .content;

                                DocsDb::insert(backend, metadata.id, &document)
                                    .map_err(SaveDocumentError)?;
                            }
                        }
                    }
                    Some(local_changes) => {
                        // It's dirty, merge changes

                        // You deleted this file locally, send this to the server
                        if local_changes.deleted && !metadata.deleted {
                            // If we wanted to recover files that were deleted locally but things
                            // on the server changed, we could do so here.

                            // straightforward metadata merge
                        } else if !local_changes.deleted && !metadata.deleted {
                            // We renamed it locally
                            if let Some(renamed_locally) = local_changes.renamed {
                                // Check if both renamed, if so, server wins
                                if metadata.name != renamed_locally.old_value {
                                    ChangeDb::untrack_rename(backend, metadata.id)
                                        .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                } else {
                                    metadata.name = local_metadata.name.clone();
                                }
                            }

                            // We moved it locally
                            if let Some(moved_locally) = local_changes.moved {
                                // Check if both moved, if so server wins
                                if metadata.parent != moved_locally.old_value {
                                    ChangeDb::untrack_move(backend, metadata.id)
                                        .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                } else {
                                    metadata.parent = local_metadata.parent;
                                    metadata.folder_access_keys = local_metadata.folder_access_keys;
                                }
                            }

                            if local_changes.new {
                                error!("Server has modified a file this client has marked as new! This should not be possible. id: {}", metadata.id);
                            }

                            if let Some(edited_locally) = local_changes.content_edited {
                                info!("Content conflict for: {}", metadata.id);
                                if local_metadata.content_version != metadata.content_version {
                                    if metadata.file_type == Folder {
                                        // Should be unreachable
                                        error!("Not only was a folder edited, it was edited according to the server as well. This should not be possible, id: {}", metadata.id);
                                    }

                                    if metadata.name.ends_with(".md")
                                        || metadata.name.ends_with(".txt")
                                    {
                                        let common_ancestor = {
                                            let compressed_common_ancestor =
                                                FileCrypto::user_read_document(
                                                    &account,
                                                    &edited_locally.old_value,
                                                    &edited_locally.access_info,
                                                )
                                                .map_err(DecryptingOldVersionForMergeError)?;

                                            FileCompression::decompress(&compressed_common_ancestor)
                                                .map_err(DecompressingForMergeError)?
                                        };

                                        let current_version =
                                            Files::read_document(backend, metadata.id)
                                                .map_err(ReadingCurrentVersionError)?;

                                        let server_version = {
                                            let server_document = ApiClient::request(
                                                &account,
                                                GetDocumentRequest {
                                                    id: metadata.id,
                                                    content_version: metadata.content_version,
                                                },
                                            )
                                            .map_err(WorkExecutionError::from)?
                                            .content;

                                            let compressed_server_version =
                                                FileCrypto::user_read_document(
                                                    &account,
                                                    &server_document,
                                                    &edited_locally.access_info,
                                                )
                                                .map_err(DecryptingOldVersionForMergeError)?;
                                            // This assumes that a file is never re-keyed.

                                            FileCompression::decompress(&compressed_server_version)
                                                .map_err(DecompressingForMergeError)?
                                        };

                                        let result = match diffy::merge_bytes(
                                            &common_ancestor,
                                            &current_version,
                                            &server_version,
                                        ) {
                                            Ok(no_conflicts) => no_conflicts,
                                            Err(conflicts) => conflicts,
                                        };

                                        Files::write_document(backend, metadata.id, &result)
                                            .map_err(WritingMergedFileError)?;
                                    } else {
                                        // Create a new file
                                        let new_file = DefaultFileService::create(
                                            backend,
                                            &format!(
                                                "{}-CONTENT-CONFLICT-{}",
                                                &local_metadata.name, local_metadata.id
                                            ),
                                            local_metadata.parent,
                                            Document,
                                        )
                                        .map_err(ResolveConflictByCreatingNewFileError)?;

                                        // Copy the local copy over
                                        DocsDb::insert(
                                            backend,
                                            new_file.id,
                                            &DocsDb::get(backend, local_changes.id)
                                                .map_err(SaveDocumentError)?,
                                        )
                                        .map_err(SaveDocumentError)?;

                                        // Overwrite local file with server copy
                                        let new_content = ApiClient::request(
                                            &account,
                                            GetDocumentRequest {
                                                id: metadata.id,
                                                content_version: metadata.content_version,
                                            },
                                        )
                                        .map_err(WorkExecutionError::from)?
                                        .content;

                                        DocsDb::insert(backend, metadata.id, &new_content)
                                            .map_err(SaveDocumentError)?;

                                        // Mark content as synced
                                        ChangeDb::untrack_edit(backend, metadata.id)
                                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                    }
                                }
                            }

                            FileMetadataDb::insert(backend, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                        } else if !local_changes.deleted && metadata.deleted {
                            if metadata.file_type == Document {
                                // A deleted document
                                FileMetadataDb::non_recursive_delete(backend, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoError)?;

                                ChangeDb::delete(backend, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;

                                DocsDb::delete(backend, metadata.id).map_err(SaveDocumentError)?
                            } else {
                                // A deleted folder
                                let delete_errors =
                                    FileMetadataDb::get_and_get_children_recursively(
                                        backend,
                                        metadata.id,
                                    )
                                    .map_err(WorkExecutionError::FindingChildrenFailed)?
                                    .into_iter()
                                    .map(|file_metadata| -> Option<String> {
                                        match DocsDb::delete(backend, file_metadata.id) {
                                            Ok(_) => {
                                                match FileMetadataDb::non_recursive_delete(
                                                    backend,
                                                    metadata.id,
                                                ) {
                                                    Ok(_) => {
                                                        match ChangeDb::delete(backend, metadata.id)
                                                        {
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
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_local_change(
        backend: &Backend,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        match ChangeDb::get_local_changes(backend, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)? {
            None => debug!("Calculate work indicated there was work to be done, but ChangeDb didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
            Some(mut local_change) => { // TODO this needs to be mut because the untracks are not taking effect
                if local_change.new {
                    if metadata.file_type == Document {
                        let content = DocsDb::get(backend, metadata.id).map_err(SaveDocumentError)?;
                        let version = ApiClient::request(
            &account,
                            CreateDocumentRequest::new(&metadata, content),
                        )
                            .map_err(WorkExecutionError::from)?
                            .new_metadata_and_content_version;

                        metadata.metadata_version = version;
                        metadata.content_version = version;
                    } else {
                        let version = ApiClient::request(
            &account,
                            CreateFolderRequest::new(&metadata),
                        )
                            .map_err(WorkExecutionError::from)?
                            .new_metadata_version;

                        metadata.metadata_version = version;
                        metadata.content_version = version;
                    }

                    FileMetadataDb::insert(backend, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                    ChangeDb::untrack_new_file(backend, metadata.id)
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
                        ApiClient::request(&account, RenameDocumentRequest::new(&metadata))
                            .map_err(WorkExecutionError::from)?.new_metadata_version
                    } else {
                        ApiClient::request(&account, RenameFolderRequest::new(&metadata))
                            .map_err(WorkExecutionError::from)?.new_metadata_version
                    };
                    metadata.metadata_version = version;
                    FileMetadataDb::insert(backend, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                    ChangeDb::untrack_rename(backend, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    local_change.renamed = None;
                }

                if local_change.moved.is_some() {
                    let version = if metadata.file_type == Document {
                        ApiClient::request(&account, MoveDocumentRequest::new(&metadata)).map_err(WorkExecutionError::from)?.new_metadata_version
                    } else {
                        ApiClient::request(&account, MoveFolderRequest::new(&metadata)).map_err(WorkExecutionError::from)?.new_metadata_version
                    };

                    metadata.metadata_version = version;
                    FileMetadataDb::insert(backend, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                    ChangeDb::untrack_move(backend, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    local_change.moved = None;
                }

                if local_change.content_edited.is_some() && metadata.file_type == Document {
                    let version = ApiClient::request(&account, ChangeDocumentContentRequest{
                        id: metadata.id,
                        old_metadata_version: metadata.metadata_version,
                        new_content: DocsDb::get(backend, metadata.id).map_err(SaveDocumentError)?,
                    }).map_err(WorkExecutionError::from)?.new_metadata_and_content_version;

                    metadata.content_version = version;
                    metadata.metadata_version = version;
                    FileMetadataDb::insert(backend, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                    ChangeDb::untrack_edit(backend, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    local_change.content_edited = None;
                }

                if local_change.deleted {
                    if metadata.file_type == Document {
                        ApiClient::request(&account, DeleteDocumentRequest{ id: metadata.id }).map_err(WorkExecutionError::from)?;
                    } else {
                        ApiClient::request(&account, DeleteFolderRequest{ id: metadata.id }).map_err(WorkExecutionError::from)?;
                    }

                    ChangeDb::delete(backend, metadata.id)
                        .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    local_change.deleted = false;

                    FileMetadataDb::non_recursive_delete(backend, metadata.id)
                        .map_err(WorkExecutionError::MetadataRepoError)?; // Now it's safe to delete this locally
                }
            }
        }
        Ok(())
    }
}

impl<
        FileMetadataDb: FileMetadataRepo,
        ChangeDb: LocalChangesRepo,
        DocsDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Files: FileService,
        FileCrypto: FileEncryptionService,
        FileCompression: FileCompressionService,
    > SyncService
    for FileSyncService<
        FileMetadataDb,
        ChangeDb,
        DocsDb,
        AccountDb,
        ApiClient,
        Files,
        FileCrypto,
        FileCompression,
    >
{
    fn sync(backend: &Backend) -> Result<(), SyncError> {
        let account = AccountDb::get_account(backend).map_err(SyncError::AccountRetrievalError)?;

        let mut sync_errors: HashMap<Uuid, WorkExecutionError> = HashMap::new();

        let mut work_calculated =
            Self::calculate_work(backend).map_err(SyncError::CalculateWorkError)?;

        for _ in 0..10 {
            // Retry sync n times
            info!("Syncing");

            for work_unit in work_calculated.work_units {
                match Self::execute_work(backend, &account, work_unit.clone()) {
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
                FileMetadataDb::set_last_synced(
                    backend,
                    work_calculated.most_recent_update_from_server,
                )
                .map_err(SyncError::MetadataUpdateError)?;
            }

            work_calculated =
                Self::calculate_work(backend).map_err(SyncError::CalculateWorkError)?;

            if work_calculated.work_units.is_empty() {
                break;
            }
        }

        if sync_errors.is_empty() {
            FileMetadataDb::set_last_synced(
                backend,
                work_calculated.most_recent_update_from_server,
            )
            .map_err(SyncError::MetadataUpdateError)?;
            Ok(())
        } else {
            error!("We finished everything calculate work told us about, but still have errors, this is concerning, the errors are: {:#?}", sync_errors);
            Err(SyncError::WorkExecutionError(sync_errors))
        }
    }
    fn calculate_work(backend: &Backend) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating Work");
        let mut work_units: Vec<WorkUnit> = vec![];

        let account = AccountDb::get_account(backend).map_err(AccountRetrievalError)?;
        let last_sync = FileMetadataDb::get_last_updated(backend).map_err(GetMetadataError)?;

        let server_updates = ApiClient::request(
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

            match FileMetadataDb::maybe_get(backend, metadata.id).map_err(GetMetadataError)? {
                None => work_units.push(ServerChange { metadata }),
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

        let changes = ChangeDb::get_all_local_changes(backend).map_err(LocalChangesRepoError)?;

        for change_description in changes {
            let metadata =
                FileMetadataDb::get(backend, change_description.id).map_err(MetadataRepoError)?;

            work_units.push(LocalChange { metadata });
        }
        debug!("Work Calculated: {:#?}", work_units);

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }

    fn execute_work(
        backend: &Backend,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError> {
        match work {
            WorkUnit::LocalChange { mut metadata } => {
                Self::handle_local_change(backend, &account, &mut metadata)
            }
            WorkUnit::ServerChange { mut metadata } => {
                Self::handle_server_change(backend, &account, &mut metadata)
            }
        }
    }
}
