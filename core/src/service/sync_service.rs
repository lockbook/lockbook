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
        ApiError(client::get_updates::Error),
    }
}

error_enum! {
    enum WorkExecutionError {
        RetrievalError(repo::account_repo::Error),
        FileRetievalError(repo::file_metadata_repo::Error),
        FileContentError(repo::file_repo::Error),
        GetUpdatesError(client::get_updates::Error),
        CreateFileError(client::create_file::Error),
        GetFileError(client::get_file::Error),
        RenameFileError(client::rename_file::Error),
        MoveFileError(client::move_file::Error),
        DeleteFileError(client::delete_file::Error),
        ChangeFileContentError(client::change_file_content::Error),
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
                server_dirty_files.insert(file.clone().file_id, file.clone());
                if file.file_metadata_version > most_recent_update_from_server {
                    most_recent_update_from_server = file.file_metadata_version;
                }
            });

        let mut work_units: Vec<WorkUnit> = vec![];

        let local_dirty_files_keys = local_dirty_files
            .clone()
            .into_iter()
            .map(|f| f.file_id)
            .collect::<Vec<String>>();

        // Process intersection first
        local_dirty_files
            .clone()
            .into_iter()
            .filter(|f| server_dirty_files.contains_key(&f.file_id))
            .for_each(|client| {
                let server = server_dirty_files.get(&client.file_id).unwrap();
                work_units.extend(calculate_work_across_server_and_client(
                    server.clone(),
                    client,
                ))
            });

        // Local-only files next
        local_dirty_files
            .into_iter()
            .filter(|f| !server_dirty_files.contains_key(&f.file_id))
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
                let file_content = FileDb::get(&db, &client.file_id)?;

                let new_version = ApiClient::create_file(
                    account.username.to_string(),
                    Auth::generate_auth(&account)?,
                    client.file_id.clone(),
                    client.file_name.clone(),
                    client.file_path.clone(),
                    serde_json::to_string(&file_content)?,
                )?;

                client.file_content_version = new_version;
                client.new_file = false;
                client.content_edited_locally = false;

                FileMetadataDb::update(&db, &client)?;
                Ok(())
            }
            UpdateLocalMetadata(server_meta) => {
                let mut old_file_metadata = FileMetadataDb::get(&db, &server_meta.file_id)?;

                old_file_metadata.file_name = server_meta.file_name;
                old_file_metadata.file_path = server_meta.file_path;
                old_file_metadata.file_metadata_version = max(
                    server_meta.file_metadata_version,
                    old_file_metadata.file_metadata_version,
                );

                FileMetadataDb::update(&db, &old_file_metadata)?;
                Ok(())
            }
            PullFileContent(new_metadata) => {
                let file = ApiClient::get_file(new_metadata.file_id.clone())?;

                FileDb::update(&db, &new_metadata.file_id, &file)?;

                match FileMetadataDb::get(&db, &new_metadata.file_id) {
                    Ok(mut old_meta) => {
                        old_meta.file_content_version = new_metadata.file_content_version;
                        FileMetadataDb::update(&db, &old_meta)?;
                    }
                    Err(err) => match err {
                        MetadataError::FileRowMissing(_) => {
                            FileMetadataDb::update(
                                &db,
                                &ClientFileMetadata {
                                    file_id: new_metadata.file_id.clone(),
                                    file_name: new_metadata.file_name,
                                    file_path: new_metadata.file_path,
                                    file_content_version: new_metadata.file_content_version,
                                    file_metadata_version: new_metadata.file_metadata_version,
                                    new_file: false,
                                    content_edited_locally: false,
                                    metadata_edited_locally: false,
                                    deleted_locally: false,
                                },
                            )?;
                        }
                        _ => return Err(WorkExecutionError::FileRetievalError(err)),
                    },
                }

                Ok(())
            }
            DeleteLocally(client) => {
                FileMetadataDb::delete(&db, &client.file_id)?;
                FileDb::delete(&db, &client.file_id)?;
                Ok(())
            }
            PushMetadata(client) => {
                let mut metadata = client.clone();
                // TODO we don't know what changed so we'll send both for now, name and path a vote for combining name and path
                ApiClient::rename_file(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.file_id,
                    client.file_metadata_version,
                    metadata.file_name.clone(),
                )?;

                ApiClient::move_file(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    metadata.file_id.clone(),
                    client.file_metadata_version,
                    metadata.file_path.clone(),
                )?;

                metadata.metadata_edited_locally = false;
                FileMetadataDb::update(&db, &metadata)?;

                Ok(())
            }
            PushFileContent(client) => {
                // TODO until we're diffing this is just going to spin on conflicts
                let mut old_file_metadata = client.clone();
                let file_content = FileDb::get(&db, &client.file_id)?;

                let new_version = ApiClient::change_file_content(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.file_id.clone(),
                    client.file_content_version,
                    serde_json::to_string(&file_content)?,
                )?; // TODO the thing you're not handling is EditConflict!

                old_file_metadata.file_content_version = new_version;
                old_file_metadata.content_edited_locally = false;

                FileMetadataDb::update(&db, &old_file_metadata)?;

                Ok(())
            }
            PushDelete(client) => {
                ApiClient::delete_file(
                    account.username.clone(),
                    Auth::generate_auth(&account)?,
                    client.clone().file_id,
                    client.file_metadata_version,
                )?;

                FileMetadataDb::delete(&db, &client.file_id)?;
                FileDb::delete(&db, &client.file_id)?;

                Ok(())
            }
            PullMergePush(new_metadata) => {
                // TODO until we're diffing, this is just going to be a pull file
                let file = ApiClient::get_file(new_metadata.file_id.clone())?;

                FileDb::update(&db, &new_metadata.file_id, &file)?;

                match FileMetadataDb::get(&db, &new_metadata.file_id) {
                    Ok(mut old_meta) => {
                        old_meta.file_content_version = new_metadata.file_content_version;
                        FileMetadataDb::update(&db, &old_meta)?;
                    }
                    Err(err) => match err {
                        MetadataError::FileRowMissing(_) => {
                            FileMetadataDb::update(
                                &db,
                                &ClientFileMetadata {
                                    file_id: new_metadata.file_id.clone(),
                                    file_name: new_metadata.file_name,
                                    file_path: new_metadata.file_path,
                                    file_content_version: new_metadata.file_content_version,
                                    file_metadata_version: new_metadata.file_metadata_version,
                                    new_file: false,
                                    content_edited_locally: false,
                                    metadata_edited_locally: false,
                                    deleted_locally: false,
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
                let mut old_file_metadata = FileMetadataDb::get(&db, &server_meta.file_id)?;

                old_file_metadata.file_name = server_meta.file_name;
                old_file_metadata.file_path = server_meta.file_path;
                old_file_metadata.file_metadata_version = max(
                    server_meta.file_metadata_version,
                    old_file_metadata.file_metadata_version,
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
        client.new_file,
        client.deleted_locally,
        client.content_edited_locally,
        client.metadata_edited_locally,
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
    let local_delete = client.deleted_locally;
    let local_edit = client.content_edited_locally;
    let local_move = client.metadata_edited_locally;
    let server_delete = server.deleted;
    let server_content_change = server.file_content_version != client.file_content_version;
    // We could consider diffing across name & path instead of doing this
    let server_move = server.file_metadata_version != client.file_metadata_version;

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
