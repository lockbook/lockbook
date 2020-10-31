use std::collections::HashMap;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

use serde::Serialize;
use sled::Db;
use uuid::Uuid;

use crate::client::Client;
use crate::model::account::Account;
use crate::model::api;
use crate::model::api::{
    ChangeDocumentContentError, CreateDocumentError, CreateFolderError, DeleteDocumentError,
    DeleteFolderError, GetDocumentError, MoveDocumentError, MoveFolderError, RenameDocumentError,
    RenameFolderError,
};
use crate::model::crypto::{DecryptedValue, SignedValue};
use crate::model::file_metadata::FileMetadata;
use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::work_unit::WorkUnit;
use crate::model::work_unit::WorkUnit::{LocalChange, ServerChange};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, document_repo, file_metadata_repo, local_changes_repo};
use crate::service::auth_service::AuthService;
use crate::service::file_encryption_service::FileEncryptionService;
use crate::service::file_service::{FileService, NewFileFromPathError};
use crate::service::sync_service::CalculateWorkError::{
    AccountRetrievalError, GetMetadataError, GetUpdatesError, LocalChangesRepoError,
    MetadataRepoError,
};
use crate::service::sync_service::WorkExecutionError::{
    AutoRenameError, DecryptingOldVersionForMergeError, DocumentChangeError, DocumentCreateError,
    DocumentDeleteError, DocumentGetError, DocumentMoveError, DocumentRenameError,
    ErrorCalculatingCurrentTime, ErrorCreatingRecoveryFile, FolderCreateError, FolderMoveError,
    FolderRenameError, ReadingCurrentVersionError, ResolveConflictByCreatingNewFileError,
    SaveDocumentError, WritingMergedFileError,
};
use crate::service::{file_encryption_service, file_service};
use crate::{client, DefaultFileService};

#[derive(Debug)]
pub enum CalculateWorkError {
    LocalChangesRepoError(local_changes_repo::DbError),
    MetadataRepoError(file_metadata_repo::Error),
    GetMetadataError(file_metadata_repo::DbError),
    AccountRetrievalError(account_repo::AccountRepoError),
    GetUpdatesError(client::ApiError<api::GetUpdatesError>),
}

// TODO standardize enum variant notation within core
#[derive(Debug)]
pub enum WorkExecutionError {
    MetadataRepoError(file_metadata_repo::DbError),
    MetadataRepoErrorOpt(file_metadata_repo::Error),
    DocumentGetError(client::ApiError<GetDocumentError>),
    DocumentRenameError(client::ApiError<RenameDocumentError>),
    FolderRenameError(client::ApiError<RenameFolderError>),
    DocumentMoveError(client::ApiError<MoveDocumentError>),
    FolderMoveError(client::ApiError<MoveFolderError>),
    DocumentCreateError(client::ApiError<CreateDocumentError>),
    FolderCreateError(client::ApiError<CreateFolderError>),
    DocumentChangeError(client::ApiError<ChangeDocumentContentError>),
    DocumentDeleteError(client::ApiError<DeleteDocumentError>),
    FolderDeleteError(client::ApiError<DeleteFolderError>),
    LocalFolderDeleteError(file_service::DeleteFolderError),
    FindingChildrenFailed(file_metadata_repo::FindingChildrenFailed),
    SaveDocumentError(document_repo::Error),
    // Delete uses this and it shouldn't
    // TODO make more general
    LocalChangesRepoError(local_changes_repo::DbError),
    AutoRenameError(file_service::DocumentRenameError),
    ResolveConflictByCreatingNewFileError(file_service::NewFileError),
    DecryptingOldVersionForMergeError(file_encryption_service::UnableToReadFileAsUser),
    ReadingCurrentVersionError(file_service::ReadDocumentError),
    WritingMergedFileError(file_service::DocumentUpdateError),
    FindingParentsForConflictingFileError(file_metadata_repo::FindingParentsFailed),
    ErrorCreatingRecoveryFile(NewFileFromPathError),
    ErrorCalculatingCurrentTime(SystemTimeError),
}

#[derive(Debug)]
pub enum SyncError {
    AccountRetrievalError(account_repo::AccountRepoError),
    CalculateWorkError(CalculateWorkError),
    WorkExecutionError(HashMap<Uuid, WorkExecutionError>),
    MetadataUpdateError(file_metadata_repo::DbError),
}

pub trait SyncService {
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError>;
    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError>;
    fn handle_server_change(
        db: &Db,
        account: &Account,
        local_change: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError>;
    fn handle_local_change(
        db: &Db,
        account: &Account,
        local_change: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError>;
    fn sync(db: &Db) -> Result<(), SyncError>;
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
    Auth: AuthService,
> {
    _metadatas: FileMetadataDb,
    _changes: ChangeDb,
    _docs: DocsDb,
    _accounts: AccountDb,
    _client: ApiClient,
    _file: Files,
    _file_crypto: FileCrypto,
    _auth: Auth,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        ChangeDb: LocalChangesRepo,
        DocsDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Files: FileService,
        FileCrypto: FileEncryptionService,
        Auth: AuthService,
    > SyncService
    for FileSyncService<
        FileMetadataDb,
        ChangeDb,
        DocsDb,
        AccountDb,
        ApiClient,
        Files,
        FileCrypto,
        Auth,
    >
{
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating Work");
        let mut work_units: Vec<WorkUnit> = vec![];

        let account = AccountDb::get_account(&db).map_err(AccountRetrievalError)?;
        let last_sync = FileMetadataDb::get_last_updated(&db).map_err(GetMetadataError)?;

        let server_updates = ApiClient::get_updates(
            &account.api_url,
            &account.username,
            &SignedValue {
                content: String::default(),
                signature: String::default(),
            },
            last_sync,
        )
        .map_err(GetUpdatesError)?;

        let mut most_recent_update_from_server: u64 = last_sync;
        for metadata in server_updates {
            if metadata.metadata_version > most_recent_update_from_server {
                most_recent_update_from_server = metadata.metadata_version;
            }

            match FileMetadataDb::maybe_get(&db, metadata.id).map_err(GetMetadataError)? {
                None => work_units.push(ServerChange { metadata }),
                Some(local_metadata) => {
                    if metadata.metadata_version != local_metadata.metadata_version {
                        work_units.push(ServerChange { metadata })
                    }
                }
            };
        }

        let changes = ChangeDb::get_all_local_changes(&db).map_err(LocalChangesRepoError)?;

        for change_description in changes {
            let metadata =
                FileMetadataDb::get(&db, change_description.id).map_err(MetadataRepoError)?;

            work_units.push(LocalChange { metadata });
        }
        debug!("Work Calculated: {:#?}", work_units);

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }

    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError> {
        match work {
            WorkUnit::LocalChange { mut metadata } => {
                Self::handle_local_change(&db, &account, &mut metadata)
            }
            WorkUnit::ServerChange { mut metadata } => {
                Self::handle_server_change(&db, &account, &mut metadata)
            }
        }
    }

    fn handle_server_change(
        db: &Db,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        // Make sure no naming conflicts occur as a result of this metadata
        let conflicting_files = FileMetadataDb::get_children(&db, metadata.parent)
            .map_err(WorkExecutionError::MetadataRepoError)?
            .into_iter()
            .filter(|potential_conflict| potential_conflict.name == metadata.name)
            .filter(|potential_conflict| potential_conflict.id != metadata.id)
            .collect::<Vec<FileMetadata>>();

        // There should only be one of these
        for conflicting_file in conflicting_files {
            Files::rename_file(
                &db,
                conflicting_file.id,
                &format!(
                    "{}-NAME-CONFLICT-{}",
                    conflicting_file.name, conflicting_file.id
                ),
            )
            .map_err(AutoRenameError)?
        }

        match FileMetadataDb::maybe_get(&db, metadata.id)
            .map_err(WorkExecutionError::MetadataRepoError)?
        {
            None => {
                if !metadata.deleted {
                    // We don't know anything about this file, just do a pull
                    FileMetadataDb::insert(&db, &metadata)
                        .map_err(WorkExecutionError::MetadataRepoError)?;
                    if metadata.file_type == Document {
                        let document = ApiClient::get_document(
                            &account.api_url,
                            metadata.id,
                            metadata.content_version,
                        )
                        .map_err(DocumentGetError)?;

                        DocsDb::insert(&db, metadata.id, &document).map_err(SaveDocumentError)?;
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
                match ChangeDb::get_local_changes(&db, metadata.id)
                    .map_err(WorkExecutionError::LocalChangesRepoError)?
                {
                    None => {
                        // It has no modifications of any sort, just update it
                        if metadata.deleted {
                            // Delete this file, server deleted it and we have no local changes
                            FileMetadataDb::non_recursive_delete(&db, metadata.id)
                                .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                            if metadata.file_type == Document {
                                DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?
                            }

                        // TODO we need to check for folders and call the file_service version
                        // of this function. But this may eliminate local edits to files
                        // so you need to iterate through all children and ensure that documents
                        // don't have any content edits, if they do those changes need to be recovered
                        } else {
                            // The normal fast forward case
                            FileMetadataDb::insert(&db, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                            if metadata.file_type == Document
                                && local_metadata.metadata_version != metadata.metadata_version
                            {
                                let document = ApiClient::get_document(
                                    &account.api_url,
                                    metadata.id,
                                    metadata.content_version,
                                )
                                .map_err(DocumentGetError)?;

                                DocsDb::insert(&db, metadata.id, &document)
                                    .map_err(SaveDocumentError)?;
                            }
                        }
                    }
                    Some(local_changes) => {
                        // It's dirty, merge changes

                        // Straightforward metadata merge
                        if !metadata.deleted {
                            // We renamed it locally
                            if let Some(renamed_locally) = local_changes.renamed {
                                // Check if both renamed, if so, server wins
                                if metadata.name != renamed_locally.old_value {
                                    ChangeDb::untrack_rename(&db, metadata.id)
                                        .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                } else {
                                    metadata.name = local_metadata.name.clone();
                                }
                            }

                            // We moved it locally
                            if let Some(moved_locally) = local_changes.moved {
                                // Check if both moved, if so server wins
                                if metadata.parent != moved_locally.old_value {
                                    ChangeDb::untrack_move(&db, metadata.id)
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
                                        let common_ansestor = FileCrypto::user_read_document(
                                            &account,
                                            &edited_locally.old_value,
                                            &edited_locally.access_info,
                                        )
                                        .map_err(DecryptingOldVersionForMergeError)?
                                        .secret;

                                        let current_version =
                                            Files::read_document(&db, metadata.id)
                                                .map_err(ReadingCurrentVersionError)?
                                                .secret;

                                        let server_version = {
                                            let server_document = ApiClient::get_document(
                                                &account.api_url,
                                                metadata.id,
                                                metadata.content_version,
                                            )
                                            .map_err(DocumentGetError)?;

                                            FileCrypto::user_read_document(
                                                &account,
                                                &server_document,
                                                &edited_locally.access_info,
                                            )
                                            .map_err(DecryptingOldVersionForMergeError)? // This assumes that a file is never re-keyed.
                                            .secret
                                        };

                                        let result = match diffy::merge(
                                            &common_ansestor,
                                            &current_version,
                                            &server_version,
                                        ) {
                                            Ok(no_conflicts) => no_conflicts,
                                            Err(conflicts) => conflicts,
                                        };

                                        Files::write_document(
                                            &db,
                                            metadata.id,
                                            &DecryptedValue::from(result),
                                        )
                                        .map_err(WritingMergedFileError)?;
                                    } else {
                                        // Create a new file
                                        let new_file = DefaultFileService::create(
                                            &db,
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
                                            &db,
                                            new_file.id,
                                            &DocsDb::get(&db, local_changes.id)
                                                .map_err(SaveDocumentError)?,
                                        )
                                        .map_err(SaveDocumentError)?;

                                        // Overwrite local file with server copy
                                        let new_content = ApiClient::get_document(
                                            &account.api_url,
                                            metadata.id,
                                            metadata.content_version,
                                        )
                                        .map_err(DocumentGetError)?;

                                        DocsDb::insert(&db, metadata.id, &new_content)
                                            .map_err(SaveDocumentError)?;

                                        // Mark content as synced
                                        ChangeDb::untrack_edit(&db, metadata.id)
                                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                    }
                                }
                            }

                            // You deleted a file, but you didn't have the most recent content, server wins
                            if local_changes.deleted
                                && local_metadata.content_version != metadata.content_version
                            {
                                // Untrack the delete
                                ChangeDb::delete_if_exists(&db, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;

                                // Get the new file contents
                                if metadata.file_type == Document
                                    && local_metadata.metadata_version != metadata.metadata_version
                                {
                                    let document = ApiClient::get_document(
                                        &account.api_url,
                                        metadata.id,
                                        metadata.content_version,
                                    )
                                    .map_err(DocumentGetError)?;

                                    DocsDb::insert(&db, metadata.id, &document)
                                        .map_err(SaveDocumentError)?;
                                }
                            }

                            FileMetadataDb::insert(&db, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                        } else if local_changes.content_edited.is_none() {
                            if metadata.file_type == Document {
                                // A deleted document
                                FileMetadataDb::non_recursive_delete(&db, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;

                                ChangeDb::delete_if_exists(&db, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;

                                DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?
                            } else {
                                // A deleted folder

                                FileMetadataDb::get_with_all_children(&db, metadata.id)
                                    .map_err(WorkExecutionError::FindingChildrenFailed)?
                                    .into_iter()
                                    .filter(|file| file.file_type == Document)
                                    .map(|file| {
                                        ChangeDb::get_local_changes(&db, file.id)
                                            .map_err(WorkExecutionError::LocalChangesRepoError)?
                                    })
                                    .flatten()
                                    .filter(|change| change.content_edited.is_some())
                                    .map(|change| change.id)
                                    .map(|id| {
                                        FileMetadataDb::get(&db, id)
                                            .map_err(WorkExecutionError::MetadataRepoError)?
                                    })
                                    .map(|file| recover_document_for_delete(&db, &account, &file));

                                Files::delete_folder(&db, metadata.id, false)
                                    .map_err(WorkExecutionError::LocalFolderDeleteError)?;

                                // TODO we need to check for folders and call the file_service version
                                // of this function. But this may eliminate local edits to files
                                // so you need to iterate through all children and ensure that documents
                                // don't have any content edits, if they do those changes need to be recovered
                            }
                        } else {
                            // server's metadata == true && there is un synced content for this file
                            recover_document_for_delete(&db, &account, &metadata)
                        }
                    }
                }
            }
        }

        fn recover_document_for_delete(
            db: &Db,
            account: &Account,
            doc_to_recover: &FileMetadata,
        ) -> Result<(), WorkExecutionError> {
            let current_version = Files::read_document(&db, doc_to_recover.id)
                .map_err(ReadingCurrentVersionError)?
                .secret;

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(ErrorCalculatingCurrentTime)?
                .as_millis();

            let new_file = Files::create_at_path(
                &db,
                &format!(
                    "{}/recovered/{}/{}/{}",
                    account.username, doc_to_recover.id, timestamp, doc_to_recover.name
                ),
            )
            .map_err(ErrorCreatingRecoveryFile)?;

            Files::write_document(&db, new_file.id, &DecryptedValue::from(current_version))
                .map_err(WritingMergedFileError)?;

            FileMetadataDb::non_recursive_delete(&db, doc_to_recover.id)
                .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
            if doc_to_recover.file_type == Document {
                DocsDb::delete(&db, doc_to_recover.id).map_err(SaveDocumentError)?
            }
            ChangeDb::delete_if_exists(&db, doc_to_recover.id)
                .map_err(WorkExecutionError::LocalChangesRepoError)?;
            Ok(())
        }

        Ok(())
    }

    fn handle_local_change(
        db: &Db,
        account: &Account,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError> {
        match ChangeDb::get_local_changes(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)? {
            None => debug!("Calculate work indicated there was work to be done, but ChangeDb didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
            Some(local_change) => {
                if local_change.new {
                    if local_change.deleted {
                        if metadata.file_type == Document {
                            error!("A new file should be removed out of local_changes repo when it is deleted! id: {:?}", metadata.id);
                        } else {
                            error!("Deferred delete of folders is not allowed! id: {:?}", metadata.id);
                        }

                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    } else if metadata.file_type == Document {
                        let content = DocsDb::get(&db, metadata.id).map_err(SaveDocumentError)?;
                        let version = ApiClient::create_document(
                            &account.api_url,
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            &metadata.name,
                            metadata.parent,
                            content.content,
                            metadata.folder_access_keys.clone(),
                        )
                            .map_err(DocumentCreateError)?;

                        metadata.metadata_version = version;
                        metadata.content_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    } else {
                        let version = ApiClient::create_folder(
                            &account.api_url,
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            &metadata.name,
                            metadata.parent,
                            metadata.folder_access_keys.clone(),
                        )
                            .map_err(FolderCreateError)?;

                        metadata.metadata_version = version;
                        metadata.content_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    }
                } else if !local_change.deleted { // not new and not deleted
                    if local_change.renamed.is_some() {
                        let version = if metadata.file_type == Document {
                            ApiClient::rename_document(
                                &account.api_url,
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                &metadata.name)
                                .map_err(DocumentRenameError)?
                        } else {
                            ApiClient::rename_folder(
                                &account.api_url,
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                &metadata.name)
                                .map_err(FolderRenameError)?
                        };
                        metadata.metadata_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::untrack_rename(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    }

                    if local_change.moved.is_some() {
                        let version = if metadata.file_type == Document {
                            ApiClient::move_document(
                                &account.api_url,
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                metadata.parent,
                                metadata.folder_access_keys.clone(),
                            ).map_err(DocumentMoveError)?
                        } else {
                            ApiClient::move_folder(
                                &account.api_url,
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                metadata.parent,
                                metadata.folder_access_keys.clone(),
                            ).map_err(FolderMoveError)?
                        };

                        metadata.metadata_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::untrack_move(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    }

                    if local_change.content_edited.is_some() && metadata.file_type == Document {
                        let version = ApiClient::change_document_content(
                            &account.api_url,
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            metadata.metadata_version,
                            DocsDb::get(&db, metadata.id).map_err(SaveDocumentError)?.content,
                        ).map_err(DocumentChangeError)?;

                        metadata.content_version = version;
                        metadata.metadata_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::untrack_edit(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    }
                } else { // not new and deleted
                    if metadata.file_type == Document {
                        ApiClient::delete_document(
                            &account.api_url,
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            metadata.metadata_version
                        ).map_err(DocumentDeleteError)?;

                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    } else {
                        error!(
                            "Deferred delete of folders is not allowed! id: {:?} this code should be unreachable",
                            metadata.id
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn sync(db: &Db) -> Result<(), SyncError> {
        let account = AccountDb::get_account(&db).map_err(SyncError::AccountRetrievalError)?;

        let mut sync_errors: HashMap<Uuid, WorkExecutionError> = HashMap::new();

        let mut work_calculated =
            Self::calculate_work(&db).map_err(SyncError::CalculateWorkError)?;

        for _ in 0..10 {
            // Retry sync n times
            info!("Syncing");

            for work_unit in work_calculated.work_units {
                match Self::execute_work(&db, &account, work_unit.clone()) {
                    Ok(_) => {
                        sync_errors.remove(&work_unit.get_metadata().id);
                        debug!("{:#?} executed successfully", work_unit)
                    }
                    Err(err) => {
                        error!("Sync error detected: {:#?} {:#?}", work_unit, err);
                        sync_errors.insert(work_unit.get_metadata().id, err);
                    }
                }
            }

            work_calculated = Self::calculate_work(&db).map_err(SyncError::CalculateWorkError)?;
        }

        if sync_errors.is_empty() {
            FileMetadataDb::set_last_synced(&db, work_calculated.most_recent_update_from_server)
                .map_err(SyncError::MetadataUpdateError)?;

            Ok(())
        } else {
            error!("We finished everything calculate work told us about, but still have errors, this is concerning, the errors are: {:#?}", sync_errors);
            Err(SyncError::WorkExecutionError(sync_errors))
        }
    }
}
