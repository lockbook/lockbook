use std::collections::HashMap;
use std::time::SystemTimeError;

use serde::Serialize;
use uuid::Uuid;

use crate::client;
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
use crate::model::local_changes::{Edited, LocalChange as LocalChangeRepoLocalChange};
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

#[derive(Debug)]
pub enum CalculateWorkError<MyBackend: Backend> {
    LocalChangesRepoError(local_changes_repo::DbError<MyBackend>),
    MetadataRepoError(file_metadata_repo::GetError<MyBackend>),
    GetMetadataError(file_metadata_repo::DbError<MyBackend>),
    AccountRetrievalError(account_repo::AccountRepoError<MyBackend>),
    GetUpdatesError(client::ApiError<api::GetUpdatesError>),
}

// TODO standardize enum variant notation within core
#[derive(Debug)]
pub enum WorkExecutionError<MyBackend: Backend> {
    MetadataRepoError(file_metadata_repo::DbError<MyBackend>),
    MetadataRepoErrorOpt(file_metadata_repo::GetError<MyBackend>),
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
    LocalFolderDeleteError(file_service::DeleteFolderError<MyBackend>),
    FindingChildrenFailed(file_metadata_repo::FindingChildrenFailed<MyBackend>),
    SaveDocumentError(document_repo::Error<MyBackend>),
    // Delete uses this and it shouldn't
    // TODO make more general
    LocalChangesRepoError(local_changes_repo::DbError<MyBackend>),
    AutoRenameError(file_service::DocumentRenameError<MyBackend>),
    ResolveConflictByCreatingNewFileError(file_service::NewFileError<MyBackend>),
    DecryptingOldVersionForMergeError(file_encryption_service::UnableToReadFileAsUser),
    DecompressingForMergeError(std::io::Error),
    ReadingCurrentVersionError(file_service::ReadDocumentError<MyBackend>),
    WritingMergedFileError(file_service::DocumentUpdateError<MyBackend>),
    FindingParentsForConflictingFileError(file_metadata_repo::FindingParentsFailed<MyBackend>),
    ErrorCreatingRecoveryFile(NewFileFromPathError<MyBackend>),
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

#[derive(Debug)]
pub enum SyncError<MyBackend: Backend> {
    AccountRetrievalError(account_repo::AccountRepoError<MyBackend>),
    CalculateWorkError(CalculateWorkError<MyBackend>),
    WorkExecutionError(HashMap<Uuid, WorkExecutionError<MyBackend>>),
    MetadataUpdateError(file_metadata_repo::DbError<MyBackend>),
}

pub trait SyncService<MyBackend: Backend> {
    fn calculate_work(
        backend: &MyBackend::Db,
    ) -> Result<WorkCalculated, CalculateWorkError<MyBackend>>;
    fn execute_work(
        backend: &MyBackend::Db,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError<MyBackend>>;
    fn sync(backend: &MyBackend::Db) -> Result<(), SyncError<MyBackend>>;
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct FileSyncService<
    FileMetadataDb: FileMetadataRepo<MyBackend>,
    ChangeDb: LocalChangesRepo<MyBackend>,
    DocsDb: DocumentRepo<MyBackend>,
    AccountDb: AccountRepo<MyBackend>,
    ApiClient: Client,
    Files: FileService<MyBackend>,
    FileCrypto: FileEncryptionService,
    FileCompression: FileCompressionService,
    MyBackend: Backend,
> {
    _metadatas: FileMetadataDb,
    _changes: ChangeDb,
    _docs: DocsDb,
    _accounts: AccountDb,
    _client: ApiClient,
    _file: Files,
    _file_crypto: FileCrypto,
    _file_compression: FileCompression,
    _backend: MyBackend,
}

impl<
        MyBackend: Backend,
        FileMetadataDb: FileMetadataRepo<MyBackend>,
        ChangeDb: LocalChangesRepo<MyBackend>,
        DocsDb: DocumentRepo<MyBackend>,
        AccountDb: AccountRepo<MyBackend>,
        ApiClient: Client,
        Files: FileService<MyBackend>,
        FileCrypto: FileEncryptionService,
        FileCompression: FileCompressionService,
    > SyncService<MyBackend>
    for FileSyncService<
        FileMetadataDb,
        ChangeDb,
        DocsDb,
        AccountDb,
        ApiClient,
        Files,
        FileCrypto,
        FileCompression,
        MyBackend,
    >
{
    fn calculate_work(
        backend: &MyBackend::Db,
    ) -> Result<WorkCalculated, CalculateWorkError<MyBackend>> {
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
        backend: &MyBackend::Db,
        account: &Account,
        work: WorkUnit,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
        match work {
            WorkUnit::LocalChange { mut metadata } => {
                Self::handle_local_change(backend, &account, &mut metadata)
            }
            WorkUnit::ServerChange { mut metadata } => {
                Self::handle_server_change(backend, &account, &mut metadata)
            }
        }
    }

    fn sync(backend: &MyBackend::Db) -> Result<(), SyncError<MyBackend>> {
        let account = AccountDb::get_account(backend).map_err(SyncError::AccountRetrievalError)?;

        let mut sync_errors: HashMap<Uuid, WorkExecutionError<MyBackend>> = HashMap::new();

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
}

/// Helper functions
impl<
        FileMetadataDb: FileMetadataRepo<MyBackend>,
        ChangeDb: LocalChangesRepo<MyBackend>,
        DocsDb: DocumentRepo<MyBackend>,
        AccountDb: AccountRepo<MyBackend>,
        ApiClient: Client,
        Files: FileService<MyBackend>,
        FileCrypto: FileEncryptionService,
        FileCompression: FileCompressionService,
        MyBackend: Backend,
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
        MyBackend,
    >
{
    /// Paths within lockbook must be unique. Prior to handling a server change we make sure that
    /// there are not going to be path conflicts. If there are, we find the file that is conflicting
    /// locally and rename it
    fn rename_local_conflicting_files(
        backend: &MyBackend::Db,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
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

        Ok(())
    }

    /// Save metadata locally, and download the file contents if this file is a Document
    fn save_file_locally(
        backend: &MyBackend::Db,
        account: &Account,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
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

            DocsDb::insert(backend, metadata.id, &document).map_err(SaveDocumentError)?;
        }

        Ok(())
    }

    fn delete_file_locally(
        backend: &MyBackend::Db,
        metadata: &FileMetadata,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
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
                FileMetadataDb::get_and_get_children_recursively(backend, metadata.id)
                    .map_err(WorkExecutionError::FindingChildrenFailed)?
                    .into_iter()
                    .map(|file_metadata| -> Option<String> {
                        match DocsDb::delete(backend, file_metadata.id) {
                            Ok(_) => {
                                match FileMetadataDb::non_recursive_delete(backend, metadata.id) {
                                    Ok(_) => match ChangeDb::delete(backend, metadata.id) {
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
                return Err(RecursiveDeleteError(delete_errors));
            }
        }

        Ok(())
    }

    fn merge_documents(
        backend: &MyBackend::Db,
        account: &Account,
        metadata: &mut FileMetadata,
        local_metadata: &FileMetadata,
        local_changes: &LocalChangeRepoLocalChange,
        edited_locally: &Edited,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
        if metadata.name.ends_with(".md") || metadata.name.ends_with(".txt") {
            let common_ancestor = {
                let compressed_common_ancestor = FileCrypto::user_read_document(
                    &account,
                    &edited_locally.old_value,
                    &edited_locally.access_info,
                )
                .map_err(DecryptingOldVersionForMergeError)?;

                FileCompression::decompress(&compressed_common_ancestor)
                    .map_err(DecompressingForMergeError)?
            };

            let current_version =
                Files::read_document(backend, metadata.id).map_err(ReadingCurrentVersionError)?;

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

                let compressed_server_version = FileCrypto::user_read_document(
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

            Files::write_document(backend, metadata.id, &result).map_err(WritingMergedFileError)?;
        } else {
            // Create a new file
            let new_file = Files::create(
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
                &DocsDb::get(backend, local_changes.id).map_err(SaveDocumentError)?,
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

            DocsDb::insert(backend, metadata.id, &new_content).map_err(SaveDocumentError)?;

            // Mark content as synced
            ChangeDb::untrack_edit(backend, metadata.id)
                .map_err(WorkExecutionError::LocalChangesRepoError)?;
        }

        Ok(())
    }

    fn merge_files(
        backend: &MyBackend::Db,
        account: &Account,
        metadata: &mut FileMetadata,
        local_metadata: &FileMetadata,
        local_changes: &LocalChangeRepoLocalChange,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
        if let Some(renamed_locally) = &local_changes.renamed {
            // Check if both renamed, if so, server wins
            if metadata.name != renamed_locally.old_value {
                ChangeDb::untrack_rename(backend, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
            } else {
                metadata.name = local_metadata.name.clone();
            }
        }

        // We moved it locally
        if let Some(moved_locally) = &local_changes.moved {
            // Check if both moved, if so server wins
            if metadata.parent != moved_locally.old_value {
                ChangeDb::untrack_move(backend, metadata.id)
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
                    &backend,
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
        backend: &MyBackend::Db,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
        Self::rename_local_conflicting_files(&backend, &metadata)?;

        match FileMetadataDb::maybe_get(backend, metadata.id)
            .map_err(WorkExecutionError::MetadataRepoError)?
        {
            None => {
                if !metadata.deleted {
                    Self::save_file_locally(&backend, &account, &metadata)?;
                } else {
                    debug!(
                        "Server deleted a file we don't know about, ignored. id: {:?}",
                        metadata.id
                    );
                }
            }
            Some(local_metadata) => {
                match ChangeDb::get_local_changes(backend, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?
                {
                    None => {
                        if metadata.deleted {
                            Self::delete_file_locally(&backend, &metadata)?;
                        } else {
                            Self::save_file_locally(&backend, &account, &metadata)?;
                        }
                    }
                    Some(local_changes) => {
                        if !local_changes.deleted && !metadata.deleted {
                            // We renamed it locally

                            Self::merge_files(
                                &backend,
                                &account,
                                metadata,
                                &local_metadata,
                                &local_changes,
                            )?;

                            FileMetadataDb::insert(backend, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                        } else if !local_changes.deleted && metadata.deleted {
                            Self::delete_file_locally(&backend, &metadata)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_local_change(
        backend: &MyBackend::Db,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError<MyBackend>> {
        match ChangeDb::get_local_changes(backend, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)? {
            None => debug!("Calculate work indicated there was work to be done, but ChangeDb didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
            Some(mut local_change) => {
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

fn work_execution_error_from_api_error_common<T, MyBackend: Backend>(
    err: ApiError<T>,
) -> Result<WorkExecutionError<MyBackend>, T> {
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

impl<MyBackend: Backend> From<ApiError<GetDocumentError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<GetDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentGetError))
    }
}

impl<MyBackend: Backend> From<ApiError<RenameDocumentError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<RenameDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentRenameError))
    }
}

impl<MyBackend: Backend> From<ApiError<RenameFolderError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<RenameFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderRenameError))
    }
}

impl<MyBackend: Backend> From<ApiError<MoveDocumentError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<MoveDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentMoveError))
    }
}

impl<MyBackend: Backend> From<ApiError<MoveFolderError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<MoveFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderMoveError))
    }
}

impl<MyBackend: Backend> From<ApiError<CreateDocumentError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<CreateDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentCreateError))
    }
}

impl<MyBackend: Backend> From<ApiError<CreateFolderError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<CreateFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderCreateError))
    }
}

impl<MyBackend: Backend> From<ApiError<ChangeDocumentContentError>>
    for WorkExecutionError<MyBackend>
{
    fn from(err: ApiError<ChangeDocumentContentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentChangeError))
    }
}

impl<MyBackend: Backend> From<ApiError<DeleteDocumentError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<DeleteDocumentError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::DocumentDeleteError))
    }
}

impl<MyBackend: Backend> From<ApiError<DeleteFolderError>> for WorkExecutionError<MyBackend> {
    fn from(err: ApiError<DeleteFolderError>) -> Self {
        ok(work_execution_error_from_api_error_common(err)
            .map_err(WorkExecutionError::FolderDeleteError))
    }
}
