use std::marker::PhantomData;

use sled::Db;

use crate::client::Client;
use crate::model::api::FileMetadata as ServerFileMetadata;

use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo::Error as MetadataError;
use crate::service;

use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo::FileRepo;

use crate::model::account::Account;
use crate::model::api::{
    ChangeDocumentContentError, CreateDocumentError, DeleteDocumentError, GetUpdatesError,
    MoveDocumentError, RenameDocumentError,
};
use crate::model::work_unit::WorkUnit;
use crate::model::work_unit::WorkUnit::{
    DeleteLocally, MergeMetadataAndPushMetadata, PullFileContent, PullMergePush, PushDelete,
    PushFileContent, PushMetadata, PushNewFile, UpdateLocalMetadata,
};
use crate::service::auth_service::AuthService;
use crate::{client, error_enum};
use std::cmp::max;
use std::collections::HashMap;

error_enum! {
    enum CalculateWorkError {
        AccountRetrievalError(repo::account_repo::Error),
        FileRetievalError(repo::file_metadata_repo::Error),
        GetUpdatesError(client::Error<GetUpdatesError>),
    }
}

error_enum! {
    enum WorkExecutionError {
        RetrievalError(repo::account_repo::Error),
        FileRetievalError(repo::file_metadata_repo::Error),
        FileContentError(repo::file_repo::Error),
        GetUpdatesError(client::Error<GetUpdatesError>),
        CreateFileError(client::Error<CreateDocumentError>),
        GetFileError(client::Error<()>),
        RenameFileError(client::Error<RenameDocumentError>),
        MoveFileError(client::Error<MoveDocumentError>),
        DeleteFileError(client::Error<DeleteDocumentError>),
        ChangeDocumentContentError(client::Error<ChangeDocumentContentError>),
        AuthError(service::auth_service::AuthGenError),
        SerdeError(serde_json::Error),
    }
}

error_enum! {
    enum SyncError {
        AccountRetrievalError(repo::account_repo::Error),
        CalculateWorkError(CalculateWorkError),
        WorkExecutionError(WorkExecutionError),
        MetadataUpdateError(repo::file_metadata_repo::Error),
    }
}

pub trait SyncService {
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError>;
    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError>;
    fn sync(db: &Db) -> Result<(), SyncError>;
}

pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct FileSyncService<
    FileMetadataDb: FileMetadataRepo,
    FileDb: FileRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
    Auth: AuthService,
> {
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
    auth: PhantomData<Auth>,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        FileDb: FileRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Auth: AuthService,
    > SyncService for FileSyncService<FileMetadataDb, FileDb, AccountDb, ApiClient, Auth>
{
    fn calculate_work(db: &Db) -> Result<WorkCalculated, CalculateWorkError> {
        info!("Calculating work");

        let account = AccountDb::get_account(&db)?;
        let local_dirty_files = FileMetadataDb::get_all_dirty(&db)?;
        debug!("local dirty files: {:?}", local_dirty_files);

        let last_sync = FileMetadataDb::get_last_updated(&db)?;
        debug!("Last sync: {}", last_sync);
        let mut most_recent_update_from_server: u64 = last_sync;

        let mut server_dirty_files = HashMap::new();
        ApiClient::get_updates(account.username, "junk auth :(".to_string(), last_sync)?
            .into_iter()
            .for_each(|file| {
                server_dirty_files.insert(file.clone().id, file.clone());
                if file.metadata_version > most_recent_update_from_server {
                    most_recent_update_from_server = file.metadata_version;
                }
            });

        let mut work_units: Vec<WorkUnit> = vec![];

        let local_dirty_files_keys = local_dirty_files
            .clone()
            .into_iter()
            .map(|f| f.id)
            .collect::<Vec<String>>();

        // Process intersection first
        local_dirty_files
            .clone()
            .into_iter()
            .filter(|f| server_dirty_files.contains_key(&f.id))
            .for_each(|client| {
                let server = server_dirty_files.get(&client.id).unwrap();
                work_units.extend(calculate_work_across_server_and_client(
                    server.clone(),
                    client,
                ))
            });

        // Local-only files next
        local_dirty_files
            .into_iter()
            .filter(|f| !server_dirty_files.contains_key(&f.id))
            .for_each(|client| work_units.extend(calculate_work_for_local_changes(client)));

        server_dirty_files
            .into_iter()
            .filter(|(id, _)| !local_dirty_files_keys.contains(id))
            .for_each(|(id, server)| match FileMetadataDb::maybe_get(&db, &id) {
                Ok(maybe_value) => match maybe_value {
                    None => work_units.extend(vec![PullFileContent(server)]),
                    Some(client) => {
                        work_units.extend(calculate_work_across_server_and_client(server, client))
                    }
                },
                Err(err) => error!("Unexpected sled error! {:?}", err),
            });

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }

    // TODO consider operating off the db instead of functional arguments here
    // Doing this off the DB would also allow you to automatically update the last_synced
    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError> {
        match work {
            PushNewFile(client) => {
                let mut client = client;
                let new_version = ApiClient::create_document(
                    account.username.to_string(),
                    Auth::generate_auth(&account)?,
                    client.id.clone(),
                    client.name.clone(),
                    client.parent_id.clone(),
                    FileDb::get(&db, &client.id)?,
                )?;

                client.content_version = new_version;
                client.new = false;
                client.document_edited = false;

                FileMetadataDb::update(&db, &client)?;
                Ok(())
            }
            UpdateLocalMetadata(server_meta) => {
                let mut old_file_metadata = FileMetadataDb::get(&db, &server_meta.id)?;

                old_file_metadata.name = server_meta.name;
                old_file_metadata.parent_id = server_meta.parent;
                old_file_metadata.metadata_version = max(
                    server_meta.metadata_version,
                    old_file_metadata.metadata_version,
                );

                FileMetadataDb::update(&db, &old_file_metadata)?;
                Ok(())
            }
            PullFileContent(new_metadata) => {
                let file =
                    ApiClient::get_document(new_metadata.id.clone(), new_metadata.content_version)?;

                FileDb::update(&db, &new_metadata.id, &file)?;

                match FileMetadataDb::get(&db, &new_metadata.id) {
                    Ok(mut old_meta) => {
                        old_meta.content_version = new_metadata.content_version;
                        FileMetadataDb::update(&db, &old_meta)?;
                    }
                    Err(err) => match err {
                        MetadataError::FileRowMissing(_) => {
                            FileMetadataDb::update(
                                &db,
                                &ClientFileMetadata {
                                    id: new_metadata.id.clone(),
                                    name: new_metadata.name,
                                    parent_id: new_metadata.parent,
                                    content_version: new_metadata.content_version,
                                    metadata_version: new_metadata.metadata_version,
                                    new: false,
                                    document_edited: false,
                                    metadata_changed: false,
                                    deleted: false,
                                },
                            )?;
                        }
                        _ => return Err(WorkExecutionError::FileRetievalError(err)),
                    },
                }

                Ok(())
            }
            DeleteLocally(client) => {
                FileMetadataDb::delete(&db, &client.id)?;
                FileDb::delete(&db, &client.id)?;
                Ok(())
            }
            PushMetadata(client) => {
                // TODO until we're diffing this is just going to spin on conflicts
                let mut metadata = client.clone();
                // TODO we don't know what changed so we'll send both for now, name and path a vote for combining name and path
                ApiClient::rename_document(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.id,
                    client.metadata_version,
                    metadata.name.clone(),
                )?; // TODO the thing you're not handling is EditConflict!

                ApiClient::move_document(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    metadata.id.clone(),
                    client.metadata_version,
                    metadata.parent_id.clone(),
                )?; // TODO the thing you're not handling is EditConflict!

                metadata.metadata_changed = false;
                FileMetadataDb::update(&db, &metadata)?;

                Ok(())
            }
            PushFileContent(client) => {
                // TODO until we're diffing this is just going to spin on conflicts
                let mut old_file_metadata = client.clone();
                let file_content = FileDb::get(&db, &client.id)?;

                let new_version = ApiClient::move_document(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.id.clone(),
                    client.content_version,
                    serde_json::to_string(&file_content)?,
                )?; // TODO the thing you're not handling is EditConflict!

                old_file_metadata.content_version = new_version;
                old_file_metadata.document_edited = false;

                FileMetadataDb::update(&db, &old_file_metadata)?;

                Ok(())
            }
            PushDelete(client) => {
                // TODO until we're diffing this is just going to spin on conflicts
                ApiClient::delete_document(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.clone().id,
                    client.metadata_version,
                )?; // TODO the thing you're not handling is EditConflict!

                FileMetadataDb::delete(&db, &client.id)?;
                FileDb::delete(&db, &client.id)?;

                Ok(())
            }
            PullMergePush(new_metadata) => {
                // TODO until we're diffing, this is just going to be a pull file
                let file =
                    ApiClient::get_document(new_metadata.id.clone(), new_metadata.content_version)?;

                FileDb::update(&db, &new_metadata.id, &file)?;

                match FileMetadataDb::get(&db, &new_metadata.id) {
                    Ok(mut old_meta) => {
                        old_meta.content_version = new_metadata.content_version;
                        FileMetadataDb::update(&db, &old_meta)?;
                    }
                    Err(err) => match err {
                        MetadataError::FileRowMissing(_) => {
                            FileMetadataDb::update(
                                &db,
                                &ClientFileMetadata {
                                    id: new_metadata.id.clone(),
                                    name: new_metadata.name,
                                    parent_id: new_metadata.parent,
                                    content_version: new_metadata.content_version,
                                    metadata_version: new_metadata.metadata_version,
                                    new: false,
                                    document_edited: false,
                                    metadata_changed: false,
                                    deleted: false,
                                },
                            )?;
                        }
                        _ => return Err(WorkExecutionError::FileRetievalError(err)),
                    },
                }

                Ok(())
            }
            MergeMetadataAndPushMetadata(server_meta) => {
                // TODO we can't tell who changed what so this just going to be an UpdateLocalMetadata for now:
                let mut old_file_metadata = FileMetadataDb::get(&db, &server_meta.id)?;

                old_file_metadata.name = server_meta.name;
                old_file_metadata.parent_id = server_meta.parent;
                old_file_metadata.metadata_version = max(
                    server_meta.metadata_version,
                    old_file_metadata.metadata_version,
                );

                FileMetadataDb::update(&db, &old_file_metadata)?;
                Ok(())
            }
        }
    }

    // TODO add a maximum number of iterations
    fn sync(db: &Db) -> Result<(), SyncError> {
        info!("Syncing");
        let account = AccountDb::get_account(&db)?;
        let work_calculated = Self::calculate_work(&db)?;

        if work_calculated.work_units.is_empty() {
            info!("Done syncing");
            FileMetadataDb::set_last_updated(&db, work_calculated.most_recent_update_from_server)?;
            return Ok(());
        }

        for work_unit in work_calculated.work_units {
            match Self::execute_work(&db, &account, work_unit.clone()) {
                Ok(_) => debug!("{:?} executed successfully", work_unit),
                Err(err) => {
                    error!("{:?} failed: {:?}", work_unit, err);
                    return Err(SyncError::WorkExecutionError(err));
                }
            }
        }

        Self::sync(&db)
    }
}

fn calculate_work_for_local_changes(client: ClientFileMetadata) -> Vec<WorkUnit> {
    match (
        client.new,
        client.deleted,
        client.document_edited,
        client.metadata_changed,
    ) {
        (_, true, _, _) => vec![DeleteLocally(client)],
        (true, _, _, _) => vec![PushNewFile(client)],
        (_, _, true, false) => vec![PushFileContent(client)],
        (_, _, false, true) => vec![PushMetadata(client)],
        (_, _, true, true) => vec![PushFileContent(client.clone()), PushMetadata(client)],
        (false, false, false, false) => vec![],
    }
}

fn calculate_work_across_server_and_client(
    server: ServerFileMetadata,
    client: ClientFileMetadata,
) -> Vec<WorkUnit> {
    let local_delete = client.deleted;
    let local_edit = client.document_edited;
    let local_move = client.metadata_changed;
    let server_delete = server.deleted;
    let server_content_change = server.content_version != client.content_version;
    // We could consider diffing across name & path instead of doing this
    let server_move = server.metadata_version != client.metadata_version;

    match (
        local_delete,
        local_edit,
        local_move,
        server_delete,
        server_content_change,
        server_move,
    ) {
        (false, false, false, false, false, false) => vec![],
        (false, false, false, false, false, true) => vec![UpdateLocalMetadata(server)],
        (false, false, false, false, true, false) => vec![PullFileContent(server)],
        (false, false, false, true, false, false) => vec![DeleteLocally(client)],
        (false, false, true, false, false, false) => vec![PushMetadata(client)],
        (false, true, false, false, false, false) => vec![PushFileContent(client)],
        (true, false, false, false, false, false) => vec![PushDelete(client)],
        (true, true, false, false, false, false) => vec![PushDelete(client)],
        (true, false, true, false, false, false) => vec![PushDelete(client)],
        (true, false, false, true, false, false) => vec![DeleteLocally(client)],
        (true, false, false, false, true, false) => vec![PullFileContent(server)],
        (true, false, false, false, false, true) => vec![PushDelete(client)],
        (false, true, true, false, false, false) => {
            vec![PushFileContent(client.clone()), PushMetadata(client)]
        }
        (false, true, false, true, false, false) => vec![PushFileContent(client)],
        (false, true, false, false, true, false) => vec![PullMergePush(server)],
        (false, true, false, false, false, true) => {
            vec![UpdateLocalMetadata(server), PushFileContent(client)]
        }
        (false, false, true, true, false, false) => vec![DeleteLocally(client)],
        (false, false, true, false, true, false) => {
            vec![PushMetadata(client), PullFileContent(server)]
        }
        (false, false, true, false, false, true) => vec![MergeMetadataAndPushMetadata(server)],
        (false, false, false, true, true, false) => vec![DeleteLocally(client)],
        (false, false, false, true, false, true) => vec![DeleteLocally(client)],
        (false, false, false, false, true, true) => vec![PullFileContent(server)],
        (true, true, true, false, false, false) => vec![PushDelete(client)],
        (true, true, false, true, false, false) => vec![DeleteLocally(client)],
        (true, true, false, false, true, false) => vec![PullFileContent(server)],
        (true, true, false, false, false, true) => vec![PushDelete(client)],
        (true, false, true, true, false, false) => vec![DeleteLocally(client)],
        (true, false, true, false, true, false) => vec![PullFileContent(server)],
        (true, false, true, false, false, true) => vec![PushDelete(client)],
        (true, false, false, true, true, false) => vec![DeleteLocally(client)],
        (true, false, false, true, false, true) => vec![DeleteLocally(client)],
        (true, false, false, false, true, true) => vec![PullFileContent(server)],
        (false, true, true, true, false, false) => vec![DeleteLocally(client)],
        (false, true, true, false, true, false) => {
            vec![PullMergePush(server), PushMetadata(client)]
        }
        (false, true, true, false, false, true) => vec![
            MergeMetadataAndPushMetadata(server),
            PushFileContent(client),
        ],
        (false, true, false, true, true, false) => vec![PushFileContent(client)],
        (false, true, false, true, false, true) => {
            vec![UpdateLocalMetadata(server), PushFileContent(client)]
        }
        (false, true, false, false, true, true) => vec![PullMergePush(server)],
        (false, false, true, true, true, false) => vec![DeleteLocally(client)],
        (false, false, true, true, false, true) => vec![DeleteLocally(client)],
        (false, false, true, false, true, true) => vec![
            PullFileContent(server.clone()),
            MergeMetadataAndPushMetadata(server),
        ],
        (false, false, false, true, true, true) => vec![DeleteLocally(client)],
        (true, true, true, true, false, false) => vec![DeleteLocally(client)],
        (true, true, true, false, true, false) => vec![PullFileContent(server)],
        (true, true, true, false, false, true) => vec![PushDelete(client)],
        (true, true, false, true, true, false) => vec![DeleteLocally(client)],
        (true, true, false, true, false, true) => vec![DeleteLocally(client)],
        (true, true, false, false, true, true) => vec![PullFileContent(server)],
        (true, false, true, true, true, false) => vec![DeleteLocally(client)],
        (true, false, true, true, false, true) => vec![DeleteLocally(client)],
        (true, false, true, false, true, true) => vec![PullFileContent(server)],
        (true, false, false, true, true, true) => vec![DeleteLocally(client)],
        (false, true, true, true, true, false) => vec![PullMergePush(server), PushMetadata(client)],
        (false, true, true, true, false, true) => vec![PushFileContent(client)],
        (false, true, true, false, true, true) => vec![
            MergeMetadataAndPushMetadata(server.clone()),
            PullMergePush(server),
        ],
        (false, true, false, true, true, true) => {
            vec![PullMergePush(server.clone()), UpdateLocalMetadata(server)]
        }
        (false, false, true, true, true, true) => vec![DeleteLocally(client)],
        (true, true, true, true, true, false) => vec![DeleteLocally(client)],
        (true, true, true, true, false, true) => vec![DeleteLocally(client)],
        (true, true, true, false, true, true) => vec![PullFileContent(server)],
        (true, true, false, true, true, true) => vec![DeleteLocally(client)],
        (true, false, true, true, true, true) => vec![DeleteLocally(client)],
        (false, true, true, true, true, true) => vec![
            MergeMetadataAndPushMetadata(server.clone()),
            PullMergePush(server),
        ],
        (true, true, true, true, true, true) => vec![DeleteLocally(client)],
    }
}
