use std::marker::PhantomData;

use sled::Db;

use crate::client;
use crate::client::Client;
use crate::model::account::Account;
use crate::model::api;
use crate::model::crypto::SignedValue;
use crate::model::work_unit::WorkUnit;
use crate::model::work_unit::WorkUnit::{LocalChange, ServerChange};
use crate::repo::account_repo::AccountRepo;
use crate::repo::document_repo::DocumentRepo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::local_changes_repo::LocalChangesRepo;
use crate::repo::{account_repo, file_metadata_repo, local_changes_repo};
use crate::service::auth_service::AuthService;
use crate::service::sync_service::CalculateWorkError::{
    AccountRetrievalError, GetUpdatesError, LocalChangesRepoError, MetadataRepoError,
};

#[derive(Debug)]
pub enum CalculateWorkError {
    LocalChangesRepoError(local_changes_repo::DbError),
    MetadataRepoError(file_metadata_repo::Error),
    AccountRetrievalError(account_repo::Error),
    GetUpdatesError(client::Error<api::GetUpdatesError>),
}

#[derive(Debug)]
pub enum WorkExecutionError {}

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

        let changes = ChangeDb::get_all_local_changes(&db).map_err(LocalChangesRepoError)?;

        let mut work_units: Vec<WorkUnit> = vec![];

        for change_description in changes {
            let metadata =
                FileMetadataDb::get(&db, change_description.id).map_err(MetadataRepoError)?;

            work_units.push(LocalChange {
                metadata,
                change_description,
            });
        }
        debug!("Local Changes: {:#?}", work_units);

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

        Ok(WorkCalculated {
            work_units,
            most_recent_update_from_server,
        })
    }

    fn execute_work(db: &Db, account: &Account, work: WorkUnit) -> Result<(), WorkExecutionError> {
        unimplemented!()
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
