

use std::marker::PhantomData;

use sled::Db;


use crate::client;
use crate::client::Client;
use crate::model::account::Account;
use crate::model::api;

use crate::model::api::*;



use crate::model::work_unit::WorkUnit;

use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo::DocumentRepo;

use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::service;
use crate::service::auth_service::AuthService;


#[derive(Debug)]
pub enum CalculateWorkError {
    AccountRetrievalError(repo::account_repo::Error),
    FileRetievalError(repo::file_metadata_repo::DbError),
    FileMetadataError(repo::file_metadata_repo::Error),
    GetUpdatesError(client::Error<GetUpdatesError>),
}

#[derive(Debug)]
pub enum WorkExecutionError {
    RetrievalError(repo::account_repo::Error),
    FileRetievalError(repo::file_metadata_repo::DbError),
    FileMetadataError(repo::file_metadata_repo::Error),
    FileContentError(repo::document_repo::Error),
    GetUpdatesError(client::Error<GetUpdatesError>),
    CreateDocumentError(client::Error<CreateDocumentError>),
    CreateFolderError(client::Error<api::CreateFolderError>),
    GetFileError(client::Error<()>),
    RenameFileError(client::Error<RenameDocumentError>),
    MoveFileError(client::Error<MoveDocumentError>),
    DeleteFileError(client::Error<DeleteDocumentError>),
    ChangeDocumentContentError(client::Error<ChangeDocumentContentError>),
    AuthError(service::auth_service::AuthGenError),
    SerdeError(serde_json::Error),
}

#[derive(Debug)]
pub enum SyncError {
    AccountRetrievalError(repo::account_repo::Error),
    CalculateWorkError(CalculateWorkError),
    WorkExecutionError(Vec<WorkExecutionError>),
    MetadataUpdateError(repo::file_metadata_repo::Error),
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
    FileDb: DocumentRepo,
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
        FileDb: DocumentRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
        Auth: AuthService,
    > SyncService for FileSyncService<FileMetadataDb, FileDb, AccountDb, ApiClient, Auth>
{
    fn calculate_work(_db: &Db) -> Result<WorkCalculated, CalculateWorkError> {
        unimplemented!()
    }

    fn execute_work(_db: &Db, _account: &Account, _work: WorkUnit) -> Result<(), WorkExecutionError> {
        unimplemented!()
    }

    fn sync(_db: &Db) -> Result<(), SyncError> {
        unimplemented!()
    }
}