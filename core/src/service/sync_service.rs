use std::marker::PhantomData;

use sled::Db;

use crate::client::{
    ChangeFileContentRequest, Client, CreateFileRequest, GetFileRequest, GetUpdatesRequest,
};
use crate::model::client_file_metadata::ClientFileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo::FileRepo;
use crate::service::file_encryption_service;
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
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        FileRepoError(repo::file_repo::Error),
        MetadataRepoError(repo::file_metadata_repo::Error),
        SerderError(serde_json::error::Error),
        FileCreateError(file_encryption_service::FileCreationError),
        // TODO: Handle errors
        NewAccountError(client::NewAccountError),
        GetUpdatesError(client::GetUpdatesError),
        GetFileError(client::GetFileError),
        CreateFileError(client::CreateFileError),
        ChangeFileContentError(client::ChangeFileContentError),
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
        unimplemented!()
    }

    fn sync(db: &Db) -> Result<Vec<ClientFileMetadata>, Error> {
        unimplemented!()
    }
}
