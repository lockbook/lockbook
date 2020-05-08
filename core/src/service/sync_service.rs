use std::marker::PhantomData;

use sled::Db;

use crate::client::{Client, GetUpdatesRequest, ServerFileMetadata};
use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo::Error as MetadataError;

use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo::FileRepo;

use crate::service::logging_service::Logger;
use crate::service::sync_service::WorkUnit::*;
use crate::{client, error_enum};
use serde::Serialize;
use std::collections::HashMap;

error_enum! {
    enum CalculateWorkError {
        AccountRetrievalError(repo::account_repo::Error),
        FileRetievalError(repo::file_metadata_repo::Error),
        ApiError(client::GetUpdatesError),
    }
}

error_enum! {
    enum Error {
        RetrievalError(repo::account_repo::Error),
    }
}
type FileId = String;

#[derive(Serialize)]
pub enum WorkUnit {
    /// No action needs to be taken for this file
    Nop,

    /// File was created locally and doesn't exist anywhere else, push this file to the server
    PushNewFile(FileId),

    /// Server has changed metadata, lookup the corresponding ClientMetadata and apply Server's
    /// metadata transformations.
    UpdateLocalMetadata(ServerFileMetadata),

    /// Goto s3 and grab the new contents of this file, update metadata if successful
    PullFileContent(ServerFileMetadata),

    /// File and metadata is safe to delete locally now
    DeleteLocally(FileId),

    /// Inform the server of your metadata change
    PushMetadata(FileId),

    /// Inform the server of a local file edit. If push fails due to a conflict, attempt PullMergePush
    /// TODO we don't have a new metadata version or a new file content version without another getUpdates call
    PushFileContent(FileId),

    /// Inform the server of a file deletion. If successful, delete the file locally.
    PushDelete(FileId),

    /// Pull the new file, decrypt it, decrypt the file locally, merge them, and push the resulting file.
    PullMergePush(ServerFileMetadata),

    /// Compare with local metadata, merge non-conflicting changes, send changes to server,
    /// if successful update metadata locally.
    MergeMetadataAndPushMetadata(ServerFileMetadata),
}

pub trait SyncService {
    fn calculate_work(db: &Db) -> Result<Vec<WorkUnit>, CalculateWorkError>;
    fn sync(db: &Db) -> Result<Vec<WorkUnit>, Error>;
}

pub struct FileSyncService<
    Log: Logger,
    FileMetadataDb: FileMetadataRepo,
    FileDb: FileRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
> {
    log: PhantomData<Log>,
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
}

impl<
        Log: Logger,
        FileMetadataDb: FileMetadataRepo,
        FileDb: FileRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
    > SyncService for FileSyncService<Log, FileMetadataDb, FileDb, AccountDb, ApiClient>
{
    fn calculate_work(db: &Db) -> Result<Vec<WorkUnit>, CalculateWorkError> {
        let account = AccountDb::get_account(&db)?;
        let local_dirty_files = FileMetadataDb::get_all_dirty(&db)?;

        let last_sync = FileMetadataDb::get_last_updated(&db)?;

        let mut server_dirty_files = HashMap::new();
        ApiClient::get_updates(&GetUpdatesRequest {
            username: account.username,
            auth: "junk auth :(".to_string(),
            since_version: last_sync,
        })?
        .into_iter()
        .for_each(|file| {
            server_dirty_files.insert(file.file_id.clone(), file);
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
            .for_each(|(id, server)| match FileMetadataDb::get(&db, &id) {
                Ok(client) => {
                    work_units.extend(calculate_work_across_server_and_client(server, client))
                }
                Err(err) => match err {
                    MetadataError::SledError(_) => {
                        Log::error(format!("Unexpected sled error! {:?}", err))
                    }
                    MetadataError::SerdeError(_) => {
                        Log::error(format!("Unexpected sled error! {:?}", err))
                    }
                    MetadataError::FileRowMissing(_) => {
                        work_units.extend(vec![PullFileContent(server)])
                    }
                },
            });

        Ok(work_units)
    }

    fn sync(_db: &Db) -> Result<Vec<WorkUnit>, Error> {
        unimplemented!()
    }
}

fn calculate_work_for_local_changes(client: ClientFileMetadata) -> Vec<WorkUnit> {
    match (
        client.new_file,
        client.deleted_locally,
        client.content_edited_locally,
        client.metadata_edited_locally,
    ) {
        (_, true, _, _) => vec![DeleteLocally(client.file_id)],
        (true, _, _, _) => vec![PushNewFile(client.file_id)],
        (_, _, true, false) => vec![PushFileContent(client.file_id)],
        (_, _, false, true) => vec![PushMetadata(client.file_id)],
        (_, _, true, true) => vec![
            PushFileContent(client.file_id.clone()),
            PushMetadata(client.file_id),
        ],
        (false, false, false, false) => vec![Nop],
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
    let server_move = server.file_metadata_version != client.file_metadata_version;

    match (
        local_delete,
        local_edit,
        local_move,
        server_delete,
        server_content_change,
        server_move,
    ) {
        (false, false, false, false, false, false) => vec![Nop],
        (false, false, false, false, false, true) => vec![UpdateLocalMetadata(server)],
        (false, false, false, false, true, false) => vec![PullFileContent(server)],
        (false, false, false, true, false, false) => vec![DeleteLocally(client.file_id)],
        (false, false, true, false, false, false) => vec![PushMetadata(client.file_id)],
        (false, true, false, false, false, false) => vec![PushFileContent(client.file_id)],
        (true, false, false, false, false, false) => vec![PushDelete(client.file_id)],
        (true, true, false, false, false, false) => vec![PushDelete(client.file_id)],
        (true, false, true, false, false, false) => vec![PushDelete(client.file_id)],
        (true, false, false, true, false, false) => vec![DeleteLocally(client.file_id)],
        (true, false, false, false, true, false) => vec![PullFileContent(server)],
        (true, false, false, false, false, true) => vec![PushDelete(client.file_id)],
        (false, true, true, false, false, false) => vec![
            PushFileContent(client.file_id.clone()),
            PushMetadata(client.file_id),
        ],
        (false, true, false, true, false, false) => vec![PushFileContent(client.file_id)],
        (false, true, false, false, true, false) => vec![PullMergePush(server)],
        (false, true, false, false, false, true) => {
            vec![UpdateLocalMetadata(server), PushFileContent(client.file_id)]
        }
        (false, false, true, true, false, false) => vec![DeleteLocally(client.file_id)],
        (false, false, true, false, true, false) => {
            vec![PushMetadata(client.file_id), PullFileContent(server)]
        }
        (false, false, true, false, false, true) => vec![MergeMetadataAndPushMetadata(server)],
        (false, false, false, true, true, false) => vec![DeleteLocally(client.file_id)],
        (false, false, false, true, false, true) => vec![DeleteLocally(client.file_id)],
        (false, false, false, false, true, true) => vec![PullFileContent(server)],
        (true, true, true, false, false, false) => vec![PushDelete(client.file_id)],
        (true, true, false, true, false, false) => vec![DeleteLocally(client.file_id)],
        (true, true, false, false, true, false) => vec![PullFileContent(server)],
        (true, true, false, false, false, true) => vec![PushDelete(client.file_id)],
        (true, false, true, true, false, false) => vec![DeleteLocally(client.file_id)],
        (true, false, true, false, true, false) => vec![PullFileContent(server)],
        (true, false, true, false, false, true) => vec![PushDelete(client.file_id)],
        (true, false, false, true, true, false) => vec![DeleteLocally(client.file_id)],
        (true, false, false, true, false, true) => vec![DeleteLocally(client.file_id)],
        (true, false, false, false, true, true) => vec![PullFileContent(server)],
        (false, true, true, true, false, false) => vec![DeleteLocally(client.file_id)],
        (false, true, true, false, true, false) => {
            vec![PullMergePush(server), PushMetadata(client.file_id)]
        }
        (false, true, true, false, false, true) => vec![
            MergeMetadataAndPushMetadata(server),
            PushFileContent(client.file_id),
        ],
        (false, true, false, true, true, false) => vec![DeleteLocally(client.file_id)],
        (false, true, false, true, false, true) => {
            vec![UpdateLocalMetadata(server), PushFileContent(client.file_id)]
        }
        (false, true, false, false, true, true) => vec![PullMergePush(server)],
        (false, false, true, true, true, false) => vec![DeleteLocally(client.file_id)],
        (false, false, true, true, false, true) => vec![DeleteLocally(client.file_id)],
        (false, false, true, false, true, true) => vec![
            PullFileContent(server.clone()),
            MergeMetadataAndPushMetadata(server.clone()),
        ],
        (false, false, false, true, true, true) => vec![DeleteLocally(client.file_id)],
        (true, true, true, true, false, false) => vec![DeleteLocally(client.file_id)],
        (true, true, true, false, true, false) => vec![PullFileContent(server)],
        (true, true, true, false, false, true) => vec![PushDelete(client.file_id)],
        (true, true, false, true, true, false) => vec![DeleteLocally(client.file_id)],
        (true, true, false, true, false, true) => vec![DeleteLocally(client.file_id)],
        (true, true, false, false, true, true) => vec![PullFileContent(server)],
        (true, false, true, true, true, false) => vec![DeleteLocally(client.file_id)],
        (true, false, true, true, false, true) => vec![DeleteLocally(client.file_id)],
        (true, false, true, false, true, true) => vec![PullFileContent(server)],
        (true, false, false, true, true, true) => vec![DeleteLocally(client.file_id)],
        (false, true, true, true, true, false) => {
            vec![PullMergePush(server), PushMetadata(client.file_id)]
        }
        (false, true, true, true, false, true) => vec![PushFileContent(client.file_id)],
        (false, true, true, false, true, true) => vec![
            MergeMetadataAndPushMetadata(server.clone()),
            PullMergePush(server.clone()),
        ],
        (false, true, false, true, true, true) => vec![
            PullMergePush(server.clone()),
            UpdateLocalMetadata(server.clone()),
        ],
        (false, false, true, true, true, true) => vec![DeleteLocally(client.file_id)],
        (true, true, true, true, true, false) => vec![DeleteLocally(client.file_id)],
        (true, true, true, true, false, true) => vec![DeleteLocally(client.file_id)],
        (true, true, true, false, true, true) => vec![PullFileContent(server)],
        (true, true, false, true, true, true) => vec![DeleteLocally(client.file_id)],
        (true, false, true, true, true, true) => vec![DeleteLocally(client.file_id)],
        (false, true, true, true, true, true) => vec![
            MergeMetadataAndPushMetadata(server.clone()),
            PullMergePush(server.clone()),
        ],
        (true, true, true, true, true, true) => vec![DeleteLocally(client.file_id)],
    }
}
