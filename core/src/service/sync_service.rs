use std::marker::PhantomData;

use sled::Db;

use crate::client;
use crate::client::Client;
use crate::model::account::Account;
use crate::model::api;
use crate::model::api::{
    ChangeDocumentContentError, CreateDocumentError, CreateFolderError, DeleteDocumentError,
    DeleteFolderError, MoveDocumentError, MoveFolderError, RenameDocumentError, RenameFolderError,
};
use crate::model::crypto::SignedValue;

use crate::model::file_metadata::FileType::Document;

use crate::model::work_unit::WorkUnit;
use crate::model::work_unit::WorkUnit::{LocalChange, ServerChange};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, document_repo, file_metadata_repo, local_changes_repo};
use crate::service::auth_service::AuthService;
use crate::service::sync_service::CalculateWorkError::{
    AccountRetrievalError, GetUpdatesError, LocalChangesRepoError, MetadataRepoError,
};
use crate::service::sync_service::WorkExecutionError::{
    DocumentChangeError, DocumentCreateError, DocumentDeleteError, DocumentMoveError,
    DocumentRenameError, FolderCreateError, FolderDeleteError, FolderMoveError, FolderRenameError,
    GetDocumentError, SaveDocumentError,
};

#[derive(Debug)]
pub enum CalculateWorkError {
    LocalChangesRepoError(local_changes_repo::DbError),
    MetadataRepoError(file_metadata_repo::Error),
    AccountRetrievalError(account_repo::Error),
    GetUpdatesError(client::Error<api::GetUpdatesError>),
}

#[derive(Debug)]
pub enum WorkExecutionError {
    MetadataRepoError(file_metadata_repo::DbError),
    MetadataRepoErrorOpt(file_metadata_repo::Error),
    GetDocumentError(client::Error<()>),
    DocumentRenameError(client::Error<RenameDocumentError>),
    FolderRenameError(client::Error<RenameFolderError>),
    DocumentMoveError(client::Error<MoveDocumentError>),
    FolderMoveError(client::Error<MoveFolderError>),
    DocumentCreateError(client::Error<CreateDocumentError>),
    FolderCreateError(client::Error<CreateFolderError>),
    DocumentChangeError(client::Error<ChangeDocumentContentError>),
    DocumentDeleteError(client::Error<DeleteDocumentError>),
    FolderDeleteError(client::Error<DeleteFolderError>),
    SaveDocumentError(document_repo::Error), // TODO make more general
    LocalChangesRepoError(local_changes_repo::DbError),
}

#[derive(Debug)]
pub enum SyncError {
    AccountRetrievalError(account_repo::Error),
    CalculateWorkError(CalculateWorkError),
    WorkExecutionError(Vec<WorkExecutionError>),
    MetadataUpdateError(file_metadata_repo::Error),
}

pub trait SyncService {
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError>;
    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError>;
    fn sync(db: &Db) -> Result<(), SyncError>;
}

#[derive(Debug)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct FileSyncService<
    FileMetadataDb: FileMetadataRepo,
    ChangeDb: LocalChangesRepo,
    FileDb: DocumentRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Auth: AuthService,
> {
    metadatas: PhantomData<FileMetadataDb>,
    changes: PhantomData<ChangeDb>,
    files: PhantomData<FileDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
    auth: PhantomData<Auth>,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        ChangeDb: LocalChangesRepo,
        FileDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Auth: AuthService,
    > SyncService
    for FileSyncService<FileMetadataDb, ChangeDb, FileDb, AccountDb, ApiClient, Auth>
{
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating Work");
        let mut work_units: Vec<WorkUnit> = vec![];

        let account = AccountDb::get_account(&db).map_err(AccountRetrievalError)?;
        let last_sync = FileMetadataDb::get_last_updated(&db).map_err(MetadataRepoError)?;

        let server_updates = ApiClient::get_updates(
            &account.username,
            &SignedValue {
                content: String::default(),
                signature: String::default(),
            },
            last_sync,
        )
        .map_err(GetUpdatesError)?;
        debug!("Server Updates: {:#?}", server_updates);

        let mut most_recent_update_from_server: u64 = last_sync;
        for metadata in server_updates {
            if metadata.metadata_version > most_recent_update_from_server {
                most_recent_update_from_server = metadata.metadata_version;
            }

            work_units.push(ServerChange { metadata });
        }

        let changes = ChangeDb::get_all_local_changes(&db).map_err(LocalChangesRepoError)?;

        for change_description in changes {
            let metadata =
                FileMetadataDb::get(&db, change_description.id).map_err(MetadataRepoError)?;

            work_units.push(LocalChange { metadata });
        }
        debug!("Local Changes: {:#?}", work_units);

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }

    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError> {
        match work {
            WorkUnit::LocalChange { mut metadata } => {
                match ChangeDb::get_local_changes(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)? {
                    None => debug!("Calculate work indicated there was work to be done, but ChangeDb didn't give us anything. It must have been unset by a server change. id: {:?}", metadata.id),
                    Some(local_change) => {
                        if local_change.new {
                            if local_change.deleted {
                                FileMetadataDb::actually_delete(&db, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                                if metadata.file_type == Document {
                                    FileDb::delete(&db, metadata.id)
                                        .map_err(SaveDocumentError)?
                                }                                ChangeDb::delete_if_exists(&db, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
                            } else if metadata.file_type == Document {
                                let content = FileDb::get(&db, metadata.id).map_err(SaveDocumentError)?;
                                let version = ApiClient::create_document(
                                    &account.username,
                                    &SignedValue { content: "".to_string(), signature: "".to_string() },
                                    metadata.id,
                                    &metadata.name,
                                    metadata.parent,
                                    content.content,
                                    metadata.folder_access_keys.clone()
                                )
                                    .map_err(DocumentCreateError)?;

                                metadata.metadata_version = version;
                                metadata.content_version = version;

                                FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;
                            } else {
                                let version = ApiClient::create_folder(
                                    &account.username,
                                    &SignedValue { content: "".to_string(), signature: "".to_string() },
                                    metadata.id,
                                    &metadata.name,
                                    metadata.parent,
                                    metadata.folder_access_keys.clone()
                                )
                                    .map_err(FolderCreateError)?;

                                metadata.metadata_version = version;
                                FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;
                            }
                        } else if !local_change.deleted { // not new and not deleted
                            if let Some(renamed_locally) = local_change.renamed {
                                let version = if metadata.file_type == Document {
                                    let version = ApiClient::rename_document(
                                        &account.username,
                                        &SignedValue { content: "".to_string(), signature: "".to_string() },
                                        metadata.id, metadata.metadata_version,
                                        &metadata.name)
                                        .map_err(DocumentRenameError)?;
                                    metadata.content_version = version;
                                    version
                                } else {
                                    ApiClient::rename_folder(
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

                            if let Some(moved_locally) = local_change.moved {
                                let version = if metadata.file_type == Document {
                                    let version = ApiClient::move_document(
                                        &account.username,
                                        &SignedValue { content: "".to_string(), signature: "".to_string() },
                                        metadata.id, metadata.metadata_version,
                                        metadata.parent
                                    ).map_err(DocumentMoveError)?;
                                    metadata.content_version = version;
                                    version
                                } else {
                                    ApiClient::move_folder(
                                        &account.username,
                                        &SignedValue { content: "".to_string(), signature: "".to_string() },
                                        metadata.id, metadata.metadata_version,
                                        metadata.parent
                                    ).map_err(FolderMoveError)?
                                };
                                metadata.metadata_version = version;
                                FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                                ChangeDb::untrack_move(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                            }

                            if local_change.content_edited && metadata.file_type == Document {
                                let version = ApiClient::change_document_content(
                                    &account.username,
                                    &SignedValue{ content: "".to_string(), signature: "".to_string() },
                                    metadata.id,
                                    metadata.content_version,
                                    FileDb::get(&db, metadata.id).map_err(SaveDocumentError)?.content
                                ).map_err(DocumentChangeError)?;

                                metadata.content_version = version;
                                metadata.metadata_version = version;
                                FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                                ChangeDb::untrack_edit(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                            }
                        } else { // not new and deleted
                            if metadata.file_type == Document {
                                ApiClient::delete_document(
                                    &account.username,
                                    &SignedValue { content: "".to_string(), signature: "".to_string() },
                                    metadata.id,
                                    metadata.metadata_version
                                ).map_err(DocumentDeleteError)?;

                                FileMetadataDb::actually_delete(&db, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                                FileDb::delete(&db, metadata.id).map_err(SaveDocumentError)?;
                                ChangeDb::delete_if_exists(&db, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
                            } else {
                                ApiClient::delete_folder(
                                    &account.username,
                                    &SignedValue { content: "".to_string(), signature: "".to_string() },
                                    metadata.id,
                                    metadata.metadata_version
                                ).map_err(FolderDeleteError)?;

                                FileMetadataDb::actually_delete(&db, metadata.id)
                                    .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                                ChangeDb::delete_if_exists(&db, metadata.id)
                                    .map_err(WorkExecutionError::LocalChangesRepoError)?;
                            }
                        }
                    }
                }
                Ok(())
            }
            WorkUnit::ServerChange { mut metadata } => {
                match FileMetadataDb::maybe_get(&db, metadata.id)
                    .map_err(WorkExecutionError::MetadataRepoError)?
                {
                    None => {
                        if !metadata.deleted {
                            // We don't know anything about this file, just do a pull
                            FileMetadataDb::insert(&db, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                            if metadata.file_type == Document {
                                let document =
                                    ApiClient::get_document(metadata.id, metadata.content_version)
                                        .map_err(GetDocumentError)?;

                                FileDb::insert(&db, metadata.id, &document)
                                    .map_err(SaveDocumentError)?;
                            }
                        } else {
                            debug!("Server deleted a file we don't know about, ignored. id: {:?}", metadata.id);
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
                                    FileMetadataDb::actually_delete(&db, metadata.id)
                                        .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                                    if metadata.file_type == Document {
                                        FileDb::delete(&db, metadata.id)
                                            .map_err(SaveDocumentError)?
                                    }                                } else {
                                    // The normal fast forward case
                                    FileMetadataDb::insert(&db, &metadata)
                                        .map_err(WorkExecutionError::MetadataRepoError)?;
                                    if metadata.file_type == Document
                                        && local_metadata.metadata_version
                                            != metadata.metadata_version
                                    {
                                        let document = ApiClient::get_document(
                                            metadata.id,
                                            metadata.content_version,
                                        )
                                        .map_err(GetDocumentError)?;

                                        FileDb::insert(&db, metadata.id, &document)
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
                                            ChangeDb::untrack_rename(&db, metadata.id).map_err(
                                                WorkExecutionError::LocalChangesRepoError,
                                            )?;
                                        } else {
                                            metadata.name = local_metadata.name;
                                        }
                                    }

                                    // We moved it locally
                                    if let Some(moved_locally) = local_changes.moved {
                                        // Check if both moved, if so server wins
                                        if metadata.parent != moved_locally.old_value {
                                            ChangeDb::untrack_rename(&db, metadata.id).map_err(
                                                WorkExecutionError::LocalChangesRepoError,
                                            )?;
                                        } else {
                                            metadata.parent = local_metadata.parent;
                                            metadata.folder_access_keys =
                                                local_metadata.folder_access_keys;
                                        }
                                    }

                                    if local_changes.new {
                                        error!("Server has modified a file this client has marked as new! This should not be possible. id: {}", metadata.id);
                                    }

                                    if local_changes.content_edited
                                        && local_metadata.content_version
                                            != metadata.content_version
                                    {
                                        error!("Local changes conflict with server changes, implement diffing! unimplemented!() server wins for now");
                                    }

                                    // You deleted a file, but you didn't have the most recent content, server wins
                                    if local_changes.deleted
                                        && local_metadata.content_version
                                            != metadata.content_version
                                    {
                                        ChangeDb::untrack_delete(&db, metadata.id)
                                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                    }

                                    FileMetadataDb::insert(&db, &metadata)
                                        .map_err(WorkExecutionError::MetadataRepoError)?;
                                } else if !local_changes.content_edited {
                                    FileMetadataDb::actually_delete(&db, metadata.id)
                                        .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;

                                    ChangeDb::delete_if_exists(&db, metadata.id)
                                        .map_err(WorkExecutionError::LocalChangesRepoError)?;

                                    if metadata.file_type == Document {
                                        FileDb::delete(&db, metadata.id)
                                            .map_err(SaveDocumentError)?
                                    }
                                } else {
                                    error!("The server deleted this file, and you have local changes! You have to undelete this file unimplemented!() server wins for now");
                                    FileMetadataDb::actually_delete(&db, metadata.id)
                                        .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                                    if metadata.file_type == Document {
                                        FileDb::delete(&db, metadata.id)
                                            .map_err(SaveDocumentError)?
                                    }                                    ChangeDb::delete_if_exists(&db, metadata.id)
                                        .map_err(WorkExecutionError::LocalChangesRepoError)?;
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn sync(db: &Db) -> Result<(), SyncError> {
        let mut sync_errors = vec![];

        for _ in 0..10 {
            info!("Syncing");
            let account = AccountDb::get_account(&db).map_err(SyncError::AccountRetrievalError)?;
            let work_calculated =
                Self::calculate_work(&db).map_err(SyncError::CalculateWorkError)?;

            debug!("Work calculated: {:#?}", work_calculated);

            if work_calculated.work_units.is_empty() {
                info!("Done syncing");
                FileMetadataDb::set_last_updated(
                    &db,
                    work_calculated.most_recent_update_from_server,
                )
                .map_err(SyncError::MetadataUpdateError)?;
                return Ok(());
            }

            for work_unit in work_calculated.work_units {
                match Self::execute_work(&db, &account, work_unit.clone()) {
                    Ok(_) => debug!("{:#?} executed successfully", work_unit),
                    Err(err) => {
                        error!("{:?} failed: {:?}", work_unit, err);
                        sync_errors.push(err);
                    }
                }
            }
        }

        if sync_errors.is_empty() {
            Ok(())
        } else {
            Err(SyncError::WorkExecutionError(sync_errors))
        }
    }
}
