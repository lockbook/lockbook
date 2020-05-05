use std::marker::PhantomData;

use sled::Db;

use crate::client::{Client, GetUpdatesRequest};
use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;

use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo::FileRepo;

use crate::service::logging_service::Logger;
use crate::{client, error_enum};

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

pub trait SyncService {
    fn calculate_work(db: &Db) -> Result<Vec<ClientFileMetadata>, CalculateWorkError>;
    fn sync(db: &Db) -> Result<Vec<ClientFileMetadata>, Error>;
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
    fn calculate_work(db: &Db) -> Result<Vec<ClientFileMetadata>, CalculateWorkError> {
        let account = AccountDb::get_account(&db)?;
        let local_dirty_files = FileMetadataDb::get_all_dirty(&db)?;

        let last_sync = FileMetadataDb::get_last_updated(&db)?;
        let server_dirty_files = ApiClient::get_updates(&GetUpdatesRequest {
            username: account.username,
            auth: "junk auth :(".to_string(),
            since_version: last_sync
        })?;

        let

        unimplemented!()
    }

    fn sync(_db: &Db) -> Result<Vec<ClientFileMetadata>, Error> {
        unimplemented!()
    }
}
