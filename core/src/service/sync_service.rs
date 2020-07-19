use std::collections::HashMap;
use std::marker::PhantomData;

use sled::Db;
use uuid::Uuid;

use crate::client;
use crate::client::Client;
use crate::model::account::Account;
use crate::model::api;
use crate::model::api::{
    ChangeDocumentContentError, CreateDocumentError, CreateFolderError, DeleteDocumentError,
    DeleteFolderError, GetDocumentError, MoveDocumentError, MoveFolderError, RenameDocumentError,
    RenameFolderError,
};
use crate::model::crypto::SignedValue;
use crate::model::file_metadata::FileMetadata;
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
    AccountRetrievalError, GetMetadataError, GetUpdatesError, LocalChangesRepoError,
    MetadataRepoError,
};
use crate::service::sync_service::WorkExecutionError::{
    DocumentChangeError, DocumentCreateError, DocumentDeleteError, DocumentGetError,
    DocumentMoveError, DocumentRenameError, FolderCreateError, FolderDeleteError, FolderMoveError,
    FolderRenameError, SaveDocumentError,
};

#[derive(Debug)]
pub enum CalculateWorkError {
    LocalChangesRepoError(local_changes_repo::DbError),
    MetadataRepoError(file_metadata_repo::Error),
    GetMetadataError(file_metadata_repo::DbError),
    AccountRetrievalError(account_repo::AccountRepoError),
    GetUpdatesError(client::Error<api::GetUpdatesError>),
}

#[derive(Debug)]
pub enum WorkExecutionError {
    MetadataRepoError(file_metadata_repo::DbError),
    MetadataRepoErrorOpt(file_metadata_repo::Error),
    DocumentGetError(client::Error<GetDocumentError>),
    DocumentRenameError(client::Error<RenameDocumentError>),
    FolderRenameError(client::Error<RenameFolderError>),
    DocumentMoveError(client::Error<MoveDocumentError>),
    FolderMoveError(client::Error<MoveFolderError>),
    DocumentCreateError(client::Error<CreateDocumentError>),
    FolderCreateError(client::Error<CreateFolderError>),
    DocumentChangeError(client::Error<ChangeDocumentContentError>),
    DocumentDeleteError(client::Error<DeleteDocumentError>),
    FolderDeleteError(client::Error<DeleteFolderError>),
    SaveDocumentError(document_repo::Error),
    // TODO make more general
    LocalChangesRepoError(local_changes_repo::DbError),
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
        local_change: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError>;
    fn handle_local_change(
        db: &Db,
        account: &Account,
        local_change: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError>;
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
    DocsDb: DocumentRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Auth: AuthService,
> {
    metadatas: PhantomData<FileMetadataDb>,
    changes: PhantomData<ChangeDb>,
    docs: PhantomData<DocsDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
    auth: PhantomData<Auth>,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        ChangeDb: LocalChangesRepo,
        DocsDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Auth: AuthService,
    > SyncService
    for FileSyncService<FileMetadataDb, ChangeDb, DocsDb, AccountDb, ApiClient, Auth>
{
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating Work");
        let mut work_units: Vec<WorkUnit> = vec![];

        let account = AccountDb::get_account(&db).map_err(AccountRetrievalError)?;
        let last_sync = FileMetadataDb::get_last_updated(&db).map_err(GetMetadataError)?;

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
        debug!("Local Changes: {:#?}", work_units);

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
                Self::handle_server_change(&db, &mut metadata)
            }
        }
    }

    fn handle_server_change(
        db: &Db,
        metadata: &mut FileMetadata,
    ) -> Result<(), WorkExecutionError> {
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
                            FileMetadataDb::actually_delete(&db, metadata.id)
                                .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                            if metadata.file_type == Document {
                                DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?
                            }
                        } else {
                            // The normal fast forward case
                            FileMetadataDb::insert(&db, &metadata)
                                .map_err(WorkExecutionError::MetadataRepoError)?;
                            if metadata.file_type == Document
                                && local_metadata.metadata_version != metadata.metadata_version
                            {
                                let document =
                                    ApiClient::get_document(metadata.id, metadata.content_version)
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
                                    metadata.name = local_metadata.name;
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

                            if local_changes.content_edited
                                && local_metadata.content_version != metadata.content_version
                            {
                                error!("Local changes conflict with server changes, implement diffing! unimplemented!() server wins for now");
                            }

                            // You deleted a file, but you didn't have the most recent content, server wins
                            if local_changes.deleted
                                && local_metadata.content_version != metadata.content_version
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
                                DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?
                            }
                        } else {
                            error!("The server deleted this file, and you have local changes! You have to undelete this file unimplemented!() server wins for now");
                            FileMetadataDb::actually_delete(&db, metadata.id)
                                .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                            if metadata.file_type == Document {
                                DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?
                            }
                            ChangeDb::delete_if_exists(&db, metadata.id)
                                .map_err(WorkExecutionError::LocalChangesRepoError)?;
                        }
                    }
                }
            }
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
                        FileMetadataDb::actually_delete(&db, metadata.id)
                            .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                        if metadata.file_type == Document {
                            DocsDb::delete(&db, metadata.id)
                                .map_err(SaveDocumentError)?
                        }

                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    } else if metadata.file_type == Document {
                        let content = DocsDb::get(&db, metadata.id).map_err(SaveDocumentError)?;
                        let version = ApiClient::create_document(
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
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                &metadata.name)
                                .map_err(DocumentRenameError)?
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

                    if local_change.moved.is_some() {
                        let version = if metadata.file_type == Document {
                            ApiClient::move_document(
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                metadata.parent,
                                metadata.folder_access_keys.clone()
                            ).map_err(DocumentMoveError)?
                        } else {
                            ApiClient::move_folder(
                                &account.username,
                                &SignedValue { content: "".to_string(), signature: "".to_string() },
                                metadata.id, metadata.metadata_version,
                                metadata.parent,
                                metadata.folder_access_keys.clone()
                            ).map_err(FolderMoveError)?
                        };

                        metadata.metadata_version = version;
                        FileMetadataDb::insert(&db, &metadata).map_err(WorkExecutionError::MetadataRepoError)?;

                        ChangeDb::untrack_move(&db, metadata.id).map_err(WorkExecutionError::LocalChangesRepoError)?;
                    }

                    if local_change.content_edited && metadata.file_type == Document {
                        let version = ApiClient::change_document_content(
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
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            metadata.metadata_version,
                        ).map_err(DocumentDeleteError)?;

                        FileMetadataDb::actually_delete(&db, metadata.id)
                            .map_err(WorkExecutionError::MetadataRepoErrorOpt)?;
                        DocsDb::delete(&db, metadata.id).map_err(SaveDocumentError)?;
                        ChangeDb::delete_if_exists(&db, metadata.id)
                            .map_err(WorkExecutionError::LocalChangesRepoError)?;
                    } else {
                        ApiClient::delete_folder(
                            &account.username,
                            &SignedValue { content: "".to_string(), signature: "".to_string() },
                            metadata.id,
                            metadata.metadata_version,
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

    fn sync(db: &Db) -> Result<(), SyncError> {
        // If you create a/b/c/file.txt and sync, if it syncs out of order this could cause errors
        // This isn't an issue as we have a retry policy built in, taking the approach that sync shall
        // eventually succeed. But there may be genuine errors (renamed to an invalid name) that aren't
        // simply retryable. Keep track of every error for every file and there's only a problem if we
        // were never able to sync a file.
        let mut sync_errors: HashMap<Uuid, WorkExecutionError> = HashMap::new();

        for _ in 0..10 {
            // Retry sync n times
            info!("Syncing");
            let account = AccountDb::get_account(&db).map_err(SyncError::AccountRetrievalError)?;
            let work_calculated =
                Self::calculate_work(&db).map_err(SyncError::CalculateWorkError)?;

            debug!("Work calculated: {:#?}", work_calculated);

            if work_calculated.work_units.is_empty() {
                info!("Done syncing");
                if sync_errors.is_empty() {
                    FileMetadataDb::set_last_synced(
                        &db,
                        work_calculated.most_recent_update_from_server,
                    )
                    .map_err(SyncError::MetadataUpdateError)?;
                    return Ok(());
                } else {
                    error!("We finished everything calculate work told us about, but still have errors, this is concerning, the errors are: {:#?}", sync_errors);
                    return Err(SyncError::WorkExecutionError(sync_errors));
                }
            }

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
        }

        if sync_errors.is_empty() {
            Ok(())
        } else {
            Err(SyncError::WorkExecutionError(sync_errors))
        }
    }
}
