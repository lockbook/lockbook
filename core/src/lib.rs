#[macro_use]
extern crate log;
extern crate reqwest;

use std::env;
use std::path::Path;

use serde::Serialize;
pub use sled::Db;
use uuid::Uuid;

use crate::client::{Client, ClientImpl, Error};
use crate::model::account::Account;
use crate::model::api;
use crate::model::api::{
    ChangeDocumentContentError, CreateDocumentError, CreateFolderError, DeleteDocumentError,
    DeleteFolderError, FileUsage, GetDocumentError, GetPublicKeyError, GetUpdatesError,
    MoveDocumentError, MoveFolderError, NewAccountError, RenameDocumentError, RenameFolderError,
};
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::repo::account_repo::{AccountRepo, AccountRepoError, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::db_version_repo::DbVersionRepoImpl;
use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};
use crate::repo::file_metadata_repo::{
    DbError, FileMetadataRepo, FileMetadataRepoImpl, Filter, FindingParentsFailed,
};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::repo::{document_repo, file_metadata_repo};
use crate::service::account_service::AccountExportError as ASAccountExportError;
use crate::service::account_service::{
    AccountCreationError, AccountImportError, AccountService, AccountServiceImpl,
};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::db_state_service;
use crate::service::db_state_service::{DbStateService, DbStateServiceImpl, State};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{
    DocumentRenameError, FileMoveError, ReadDocumentError as FSReadDocumentError,
};
use crate::service::file_service::{
    DocumentUpdateError, FileService, FileServiceImpl, NewFileError, NewFileFromPathError,
};
use crate::service::sync_service::{
    CalculateWorkError as SSCalculateWorkError, SyncError, WorkExecutionError,
};
use crate::service::sync_service::{FileSyncService, SyncService, WorkCalculated};
use crate::CreateAccountError::{CouldNotReachServer, InvalidUsername, UsernameTaken};
use crate::CreateFileAtPathError::{
    DocumentTreatedAsFolder, FileAlreadyExists, NoRoot, PathContainsEmptyFile,
    PathDoesntStartWithRoot,
};
use crate::GetFileByPathError::NoFileAtThatPath;
use crate::ImportError::{AccountDoesNotExist, AccountStringCorrupted, UsernamePKMismatch};
use crate::WriteToDocumentError::{FileDoesNotExist, FolderTreatedAsDocument};
use std::str::FromStr;

pub mod c_interface;
pub mod client;
pub mod java_interface;
pub mod loggers;
pub mod model;
pub mod repo;
pub mod service;

pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
static DB_NAME: &str = "lockbook.sled";
static LOG_FILE: &str = "lockbook.log";

pub type DefaultCrypto = RsaImpl;
pub type DefaultSymmetric = AesImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultClient = ClientImpl;
pub type DefaultAccountRepo = AccountRepoImpl;
pub type DefaultDbVersionRepo = DbVersionRepoImpl;
pub type DefaultDbStateService = DbStateServiceImpl<DefaultAccountRepo, DefaultDbVersionRepo>;
pub type DefaultClock = ClockImpl;
pub type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultLocalChangesRepo = LocalChangesRepoImpl;
pub type DefaultDocumentRepo = DocumentRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultLocalChangesRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultFileService,
    DefaultFileEncryptionService,
    DefaultAuthService,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultLocalChangesRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;

#[derive(Debug, Serialize)]
pub enum InitLoggerError {
    Unexpected(String),
}

pub fn init_logger(log_path: &Path) -> Result<(), InitLoggerError> {
    let print_colors = env::var("LOG_NO_COLOR").is_err();
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| log::LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or_else(|| log::LevelFilter::Debug);

    loggers::init(log_path, LOG_FILE.to_string(), print_colors)
        .map_err(|err| InitLoggerError::Unexpected(format!("IO Error: {:#?}", err)))?
        .level(log::LevelFilter::Warn)
        .level_for("lockbook_core", lockbook_log_level)
        .apply()
        .map_err(|err| InitLoggerError::Unexpected(format!("{:#?}", err)))?;
    info!("Logger initialized! Path: {:?}", log_path);
    Ok(())
}

pub fn connect_to_db(config: &Config) -> Result<Db, String> {
    let db = DefaultDbProvider::connect_to_db(&config).map_err(|err| {
        format!(
            "Could not connect to db, config: {:#?}, error: {:#?}",
            &config, err
        )
    })?;

    Ok(db)
}

#[derive(Debug, Serialize)]
pub enum GetStateError {
    UnexpectedError(String),
}

pub fn get_db_state(config: &Config) -> Result<State, GetStateError> {
    let db = connect_to_db(&config).map_err(GetStateError::UnexpectedError)?;

    match DefaultDbStateService::get_state(&db) {
        Ok(state) => Ok(state),
        Err(err) => Err(GetStateError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum MigrationError {
    StateRequiresCleaning,
    UnexpectedError(String),
}

pub fn migrate_db(config: &Config) -> Result<(), MigrationError> {
    let db = connect_to_db(&config).map_err(MigrationError::UnexpectedError)?;

    match DefaultDbStateService::perform_migration(&db) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            db_state_service::MigrationError::StateRequiresClearing => {
                Err(MigrationError::StateRequiresCleaning)
            }
            db_state_service::MigrationError::RepoError(_) => {
                Err(MigrationError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum CreateAccountError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
    UnexpectedError(String),
}

pub fn create_account(
    config: &Config,
    username: &str,
    api_url: &str,
) -> Result<(), CreateAccountError> {
    let db = connect_to_db(&config).map_err(CreateAccountError::UnexpectedError)?;

    match DefaultAccountService::create_account(&db, username, api_url) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountCreationError::AccountExistsAlready => {
                Err(CreateAccountError::AccountExistsAlready)
            }
            AccountCreationError::ApiError(network) => match network {
                Error::Api(api_err) => match api_err {
                    NewAccountError::UsernameTaken => Err(UsernameTaken),
                    NewAccountError::InvalidUsername => Err(InvalidUsername),
                    NewAccountError::ClientUpdateRequired => {
                        Err(CreateAccountError::ClientUpdateRequired)
                    }
                    NewAccountError::InternalError
                    | NewAccountError::InvalidAuth
                    | NewAccountError::ExpiredAuth
                    | NewAccountError::InvalidPublicKey
                    | NewAccountError::InvalidUserAccessKey
                    | NewAccountError::FileIdTaken => Err(CreateAccountError::UnexpectedError(
                        format!("{:#?}", api_err),
                    )),
                },
                Error::SendFailed(_) => Err(CouldNotReachServer),
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => Err(
                    CreateAccountError::UnexpectedError(format!("{:#?}", network)),
                ),
            },
            AccountCreationError::KeyGenerationError(_)
            | AccountCreationError::AccountRepoError(_)
            | AccountCreationError::FolderError(_)
            | AccountCreationError::MetadataRepoError(_)
            | AccountCreationError::KeySerializationError(_)
            | AccountCreationError::AccountRepoDbError(_)
            | AccountCreationError::AuthGenFailure(_) => {
                Err(CreateAccountError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum ImportError {
    AccountStringCorrupted,
    AccountExistsAlready,
    AccountDoesNotExist,
    UsernamePKMismatch,
    CouldNotReachServer,
    ClientUpdateRequired,
    UnexpectedError(String),
}

pub fn import_account(config: &Config, account_string: &str) -> Result<(), ImportError> {
    let db = connect_to_db(&config).map_err(ImportError::UnexpectedError)?;

    match DefaultAccountService::import_account(&db, account_string) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountImportError::AccountStringCorrupted(_)
            | AccountImportError::AccountStringFailedToDeserialize(_)
            | AccountImportError::InvalidPrivateKey(_) => Err(AccountStringCorrupted),
            AccountImportError::AccountExistsAlready => Err(ImportError::AccountExistsAlready),
            AccountImportError::PublicKeyMismatch => Err(UsernamePKMismatch),
            AccountImportError::FailedToVerifyAccountServerSide(client_err) => match client_err {
                Error::SendFailed(_) => Err(ImportError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    GetPublicKeyError::UserNotFound => Err(AccountDoesNotExist),
                    GetPublicKeyError::ClientUpdateRequired => {
                        Err(ImportError::ClientUpdateRequired)
                    }
                    GetPublicKeyError::InvalidUsername | GetPublicKeyError::InternalError => {
                        Err(ImportError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ImportError::UnexpectedError(format!("{:#?}", client_err)))
                }
            },
            AccountImportError::PersistenceError(_) | AccountImportError::AccountRepoDbError(_) => {
                Err(ImportError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum AccountExportError {
    NoAccount,
    UnexpectedError(String),
}

pub fn export_account(config: &Config) -> Result<String, AccountExportError> {
    let db = connect_to_db(&config).map_err(AccountExportError::UnexpectedError)?;

    match DefaultAccountService::export_account(&db) {
        Ok(account_string) => Ok(account_string),
        Err(err) => match err {
            ASAccountExportError::AccountRetrievalError(db_err) => match db_err {
                AccountRepoError::NoAccount => Err(AccountExportError::NoAccount),
                AccountRepoError::SerdeError(_) | AccountRepoError::SledError(_) => Err(
                    AccountExportError::UnexpectedError(format!("{:#?}", db_err)),
                ),
            },
            ASAccountExportError::AccountStringFailedToSerialize(_) => {
                Err(AccountExportError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum GetAccountError {
    NoAccount,
    UnexpectedError(String),
}

pub fn get_account(config: &Config) -> Result<Account, GetAccountError> {
    let db = connect_to_db(&config).map_err(GetAccountError::UnexpectedError)?;

    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => Ok(account),
        Err(err) => match err {
            AccountRepoError::NoAccount => Err(GetAccountError::NoAccount),
            AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                Err(GetAccountError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum CreateFileAtPathError {
    FileAlreadyExists,
    NoAccount,
    NoRoot,
    PathDoesntStartWithRoot,
    PathContainsEmptyFile,
    DocumentTreatedAsFolder,
    UnexpectedError(String),
}

pub fn create_file_at_path(
    config: &Config,
    path_and_name: &str,
) -> Result<FileMetadata, CreateFileAtPathError> {
    let db = connect_to_db(&config).map_err(CreateFileAtPathError::UnexpectedError)?;

    match DefaultFileService::create_at_path(&db, path_and_name) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            NewFileFromPathError::PathDoesntStartWithRoot => Err(PathDoesntStartWithRoot),
            NewFileFromPathError::PathContainsEmptyFile => Err(PathContainsEmptyFile),
            NewFileFromPathError::FileAlreadyExists => Err(FileAlreadyExists),
            NewFileFromPathError::NoRoot => Err(NoRoot),
            NewFileFromPathError::FailedToCreateChild(failed_to_create) => match failed_to_create {
                NewFileError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount => Err(CreateFileAtPathError::NoAccount),
                    AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                        CreateFileAtPathError::UnexpectedError(format!("{:#?}", account_error)),
                    ),
                },
                NewFileError::FileNameNotAvailable => Err(FileAlreadyExists),
                NewFileError::DocumentTreatedAsFolder => Err(DocumentTreatedAsFolder),
                NewFileError::CouldNotFindParents(_)
                | NewFileError::FileCryptoError(_)
                | NewFileError::MetadataRepoError(_)
                | NewFileError::FailedToWriteFileContent(_)
                | NewFileError::FailedToRecordChange(_)
                | NewFileError::FileNameEmpty
                | NewFileError::FileNameContainsSlash => Err(
                    CreateFileAtPathError::UnexpectedError(format!("{:#?}", failed_to_create)),
                ),
            },
            NewFileFromPathError::FailedToRecordChange(_) | NewFileFromPathError::DbError(_) => {
                Err(CreateFileAtPathError::UnexpectedError(format!(
                    "{:#?}",
                    err
                )))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum WriteToDocumentError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDocument,
    UnexpectedError(String),
}

pub fn write_document(
    config: &Config,
    id: Uuid,
    content: &DecryptedValue,
) -> Result<(), WriteToDocumentError> {
    let db = connect_to_db(&config).map_err(WriteToDocumentError::UnexpectedError)?;

    match DefaultFileService::write_document(&db, id, &content) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                    WriteToDocumentError::UnexpectedError(format!("{:#?}", account_err)),
                ),
                AccountRepoError::NoAccount => Err(WriteToDocumentError::NoAccount),
            },
            DocumentUpdateError::CouldNotFindFile => Err(FileDoesNotExist),
            DocumentUpdateError::FolderTreatedAsDocument => Err(FolderTreatedAsDocument),
            DocumentUpdateError::CouldNotFindParents(_)
            | DocumentUpdateError::FileCryptoError(_)
            | DocumentUpdateError::DocumentWriteError(_)
            | DocumentUpdateError::DbError(_)
            | DocumentUpdateError::FetchOldVersionError(_)
            | DocumentUpdateError::DecryptOldVersionError(_)
            | DocumentUpdateError::AccessInfoCreationError(_)
            | DocumentUpdateError::FailedToRecordChange(_) => {
                Err(WriteToDocumentError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum CreateFileError {
    NoAccount,
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameEmpty,
    FileNameContainsSlash,
    UnexpectedError(String),
}

pub fn create_file(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, CreateFileError> {
    let db = connect_to_db(&config).map_err(CreateFileError::UnexpectedError)?;

    match DefaultFileService::create(&db, name, parent, file_type) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            NewFileError::AccountRetrievalError(_) => Err(CreateFileError::NoAccount),
            NewFileError::DocumentTreatedAsFolder => Err(CreateFileError::DocumentTreatedAsFolder),
            NewFileError::CouldNotFindParents(parent_error) => match parent_error {
                FindingParentsFailed::AncestorMissing => Err(CreateFileError::CouldNotFindAParent),
                FindingParentsFailed::DbError(_) => Err(CreateFileError::UnexpectedError(format!(
                    "{:#?}",
                    parent_error
                ))),
            },
            NewFileError::FileNameNotAvailable => Err(CreateFileError::FileNameNotAvailable),
            NewFileError::FileNameEmpty => Err(CreateFileError::FileNameEmpty),
            NewFileError::FileNameContainsSlash => Err(CreateFileError::FileNameContainsSlash),
            NewFileError::FileCryptoError(_)
            | NewFileError::MetadataRepoError(_)
            | NewFileError::FailedToWriteFileContent(_)
            | NewFileError::FailedToRecordChange(_) => {
                Err(CreateFileError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum GetRootError {
    NoRoot,
    UnexpectedError(String),
}

pub fn get_root(config: &Config) -> Result<FileMetadata, GetRootError> {
    let db = connect_to_db(&config).map_err(GetRootError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_root(&db) {
        Ok(file_metadata) => match file_metadata {
            None => Err(GetRootError::NoRoot),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(GetRootError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum GetChildrenError {
    UnexpectedError(String),
}

pub fn get_children(config: &Config, id: Uuid) -> Result<Vec<FileMetadata>, GetChildrenError> {
    let db = connect_to_db(&config).map_err(GetChildrenError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_children(&db, id) {
        Ok(file_metadata_list) => Ok(file_metadata_list),
        Err(err) => Err(GetChildrenError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum GetFileByIdError {
    NoFileWithThatId,
    UnexpectedError(String),
}

pub fn get_file_by_id(config: &Config, id: Uuid) -> Result<FileMetadata, GetFileByIdError> {
    let db = connect_to_db(&config).map_err(GetFileByIdError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get(&db, id) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            file_metadata_repo::Error::FileRowMissing(_) => Err(GetFileByIdError::NoFileWithThatId),
            file_metadata_repo::Error::SledError(_) | file_metadata_repo::Error::SerdeError(_) => {
                Err(GetFileByIdError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
    UnexpectedError(String),
}

pub fn get_file_by_path(config: &Config, path: &str) -> Result<FileMetadata, GetFileByPathError> {
    let db = connect_to_db(&config).map_err(GetFileByPathError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_by_path(&db, path) {
        Ok(maybe_file_metadata) => match maybe_file_metadata {
            None => Err(NoFileAtThatPath),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(GetFileByPathError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum InsertFileError {
    UnexpectedError(String),
}

pub fn insert_file(config: &Config, file_metadata: FileMetadata) -> Result<(), InsertFileError> {
    let db = connect_to_db(&config).map_err(InsertFileError::UnexpectedError)?;

    match FileMetadataRepoImpl::insert(&db, &file_metadata) {
        Ok(()) => Ok(()),
        Err(err) => Err(InsertFileError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum DeleteFileError {
    NoFileWithThatId,
    UnexpectedError(String),
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), DeleteFileError> {
    let db = connect_to_db(&config).map_err(DeleteFileError::UnexpectedError)?;

    match DocumentRepoImpl::delete(&db, id) {
        Ok(()) => Ok(()),
        Err(err) => match err {
            document_repo::Error::SledError(_) | document_repo::Error::SerdeError(_) => {
                Err(DeleteFileError::UnexpectedError(format!("{:#?}", err)))
            }
            document_repo::Error::FileRowMissing(_) => Err(DeleteFileError::NoFileWithThatId),
        },
    }
}

#[derive(Debug, Serialize)]
pub enum ReadDocumentError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
    UnexpectedError(String),
}

pub fn read_document(config: &Config, id: Uuid) -> Result<DecryptedValue, ReadDocumentError> {
    let db = connect_to_db(&config).map_err(ReadDocumentError::UnexpectedError)?;

    match DefaultFileService::read_document(&db, id) {
        Ok(decrypted) => Ok(decrypted),
        Err(err) => match err {
            FSReadDocumentError::TreatedFolderAsDocument => {
                Err(ReadDocumentError::TreatedFolderAsDocument)
            }
            FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                AccountRepoError::NoAccount => Err(ReadDocumentError::NoAccount),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                    ReadDocumentError::UnexpectedError(format!("{:#?}", account_error)),
                ),
            },
            FSReadDocumentError::CouldNotFindFile => Err(ReadDocumentError::FileDoesNotExist),
            FSReadDocumentError::DbError(_)
            | FSReadDocumentError::DocumentReadError(_)
            | FSReadDocumentError::CouldNotFindParents(_)
            | FSReadDocumentError::FileCryptoError(_) => {
                Err(ReadDocumentError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum ListPathsError {
    UnexpectedError(String),
}

pub fn list_paths(config: &Config, filter: Option<Filter>) -> Result<Vec<String>, ListPathsError> {
    let db = connect_to_db(&config).map_err(ListPathsError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_all_paths(&db, filter) {
        Ok(paths) => Ok(paths),
        Err(err) => Err(ListPathsError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum ListMetadatasError {
    UnexpectedError(String),
}

pub fn list_metadatas(config: &Config) -> Result<Vec<FileMetadata>, ListMetadatasError> {
    let db = connect_to_db(&config).map_err(ListMetadatasError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_all(&db) {
        Ok(metas) => Ok(metas),
        Err(err) => Err(ListMetadatasError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum RenameFileError {
    FileDoesNotExist,
    NewNameEmpty,
    NewNameContainsSlash,
    FileNameNotAvailable,
    CannotRenameRoot,
    UnexpectedError(String),
}

pub fn rename_file(config: &Config, id: Uuid, new_name: &str) -> Result<(), RenameFileError> {
    let db = connect_to_db(&config).map_err(RenameFileError::UnexpectedError)?;

    match DefaultFileService::rename_file(&db, id, new_name) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            DocumentRenameError::FileDoesNotExist => Err(RenameFileError::FileDoesNotExist),
            DocumentRenameError::FileNameEmpty => Err(RenameFileError::NewNameEmpty),
            DocumentRenameError::FileNameContainsSlash => {
                Err(RenameFileError::NewNameContainsSlash)
            }
            DocumentRenameError::FileNameNotAvailable => Err(RenameFileError::FileNameNotAvailable),
            DocumentRenameError::CannotRenameRoot => Err(RenameFileError::CannotRenameRoot),
            DocumentRenameError::DbError(_) | DocumentRenameError::FailedToRecordChange(_) => {
                Err(RenameFileError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum MoveFileError {
    NoAccount,
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
    CannotMoveRoot,
    UnexpectedError(String),
}

pub fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), MoveFileError> {
    let db = connect_to_db(&config).map_err(MoveFileError::UnexpectedError)?;

    match DefaultFileService::move_file(&db, id, new_parent) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            FileMoveError::DocumentTreatedAsFolder => Err(MoveFileError::DocumentTreatedAsFolder),
            FileMoveError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::NoAccount => Err(MoveFileError::NoAccount),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                    MoveFileError::UnexpectedError(format!("{:#?}", account_err)),
                ),
            },
            FileMoveError::TargetParentHasChildNamedThat => {
                Err(MoveFileError::TargetParentHasChildNamedThat)
            }
            FileMoveError::FileDoesNotExist => Err(MoveFileError::FileDoesNotExist),
            FileMoveError::TargetParentDoesNotExist => Err(MoveFileError::TargetParentDoesNotExist),
            FileMoveError::CannotMoveRoot => Err(MoveFileError::CannotMoveRoot),
            FileMoveError::DbError(_)
            | FileMoveError::FailedToRecordChange(_)
            | FileMoveError::FailedToDecryptKey(_)
            | FileMoveError::FailedToReEncryptKey(_)
            | FileMoveError::CouldNotFindParents(_) => {
                Err(MoveFileError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum SyncAllError {
    NoAccount,
    CouldNotReachServer,
    ExecuteWorkError(Vec<ExecuteWorkError>),
    UnexpectedError(String),
}

pub fn sync_all(config: &Config) -> Result<(), SyncAllError> {
    let db = connect_to_db(&config).map_err(SyncAllError::UnexpectedError)?;

    match DefaultSyncService::sync(&db) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            SyncError::AccountRetrievalError(err) => match err {
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(SyncAllError::UnexpectedError(format!("{:#?}", err)))
                }
                AccountRepoError::NoAccount => Err(SyncAllError::NoAccount),
            },
            SyncError::CalculateWorkError(err) => match err {
                SSCalculateWorkError::LocalChangesRepoError(_)
                | SSCalculateWorkError::MetadataRepoError(_)
                | SSCalculateWorkError::GetMetadataError(_) => {
                    Err(SyncAllError::UnexpectedError(format!("{:#?}", err)))
                }
                SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
                    AccountRepoError::NoAccount => Err(SyncAllError::NoAccount),
                    AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                        Err(SyncAllError::UnexpectedError(format!("{:#?}", account_err)))
                    }
                },
                SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
                    Error::SendFailed(_) => Err(SyncAllError::CouldNotReachServer),
                    Error::Serialize(_)
                    | Error::ReceiveFailed(_)
                    | Error::Deserialize(_)
                    | Error::Api(_) => {
                        Err(SyncAllError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
            },
            SyncError::WorkExecutionError(err_map) => Err(SyncAllError::ExecuteWorkError(
                err_map
                    .iter()
                    .map(|err| match err.1 {
                        WorkExecutionError::DocumentGetError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::DocumentRenameError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::FolderRenameError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::DocumentMoveError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::FolderMoveError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::DocumentCreateError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::FolderCreateError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::DocumentChangeError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::DocumentDeleteError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::FolderDeleteError(api) => match api {
                            Error::SendFailed(_) => ExecuteWorkError::CouldNotReachServer,
                            Error::Serialize(_)
                            | Error::ReceiveFailed(_)
                            | Error::Deserialize(_)
                            | Error::Api(_) => {
                                ExecuteWorkError::UnexpectedError(format!("{:#?}", api))
                            }
                        },
                        WorkExecutionError::MetadataRepoError(_)
                        | WorkExecutionError::MetadataRepoErrorOpt(_)
                        | WorkExecutionError::SaveDocumentError(_)
                        | WorkExecutionError::LocalChangesRepoError(_)
                        | WorkExecutionError::AutoRenameError(_)
                        | WorkExecutionError::DecryptingOldVersionForMergeError(_)
                        | WorkExecutionError::ReadingCurrentVersionError(_)
                        | WorkExecutionError::WritingMergedFileError(_)
                        | WorkExecutionError::FindingParentsForConflictingFileError(_)
                        | WorkExecutionError::ResolveConflictByCreatingNewFileError(_) => {
                            ExecuteWorkError::UnexpectedError(format!("{:#?}", err))
                        }
                    })
                    .collect::<Vec<_>>(),
            )),
            SyncError::MetadataUpdateError(err) => {
                Err(SyncAllError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
    UnexpectedError(String),
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, CalculateWorkError> {
    let db = connect_to_db(&config).map_err(CalculateWorkError::UnexpectedError)?;

    match DefaultSyncService::calculate_work(&db) {
        Ok(work) => Ok(work),
        Err(err) => match err {
            SSCalculateWorkError::LocalChangesRepoError(_)
            | SSCalculateWorkError::MetadataRepoError(_)
            | SSCalculateWorkError::GetMetadataError(_) => {
                Err(CalculateWorkError::UnexpectedError(format!("{:#?}", err)))
            }
            SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::NoAccount => Err(CalculateWorkError::NoAccount),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => Err(
                    CalculateWorkError::UnexpectedError(format!("{:#?}", account_err)),
                ),
            },
            SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
                Error::SendFailed(_) => Err(CalculateWorkError::CouldNotReachServer),
                Error::Api(get_updates_error) => match get_updates_error {
                    GetUpdatesError::ClientUpdateRequired => {
                        Err(CalculateWorkError::ClientUpdateRequired)
                    }
                    GetUpdatesError::InternalError
                    | GetUpdatesError::InvalidAuth
                    | GetUpdatesError::ExpiredAuth
                    | GetUpdatesError::NotPermissioned
                    | GetUpdatesError::UserNotFound
                    | GetUpdatesError::InvalidUsername => Err(CalculateWorkError::UnexpectedError(
                        format!("{:#?}", get_updates_error),
                    )),
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => Err(
                    CalculateWorkError::UnexpectedError(format!("{:#?}", api_err)),
                ),
            },
        },
    }
}

#[derive(Debug, Serialize)]
pub enum ExecuteWorkError {
    CouldNotReachServer,
    ClientUpdateRequired,
    UnexpectedError(String),
    BadAccount(GetAccountError), // FIXME: @raayan Temporary to avoid passing key through FFI
}

pub fn execute_work(
    config: &Config,
    account: &Account,
    wu: WorkUnit,
) -> Result<(), ExecuteWorkError> {
    let db = connect_to_db(&config).map_err(ExecuteWorkError::UnexpectedError)?;

    match DefaultSyncService::execute_work(&db, &account, wu) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            WorkExecutionError::DocumentGetError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    GetDocumentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    GetDocumentError::InternalError | GetDocumentError::DocumentNotFound => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::DocumentRenameError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    RenameDocumentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    RenameDocumentError::InternalError
                    | RenameDocumentError::InvalidAuth
                    | RenameDocumentError::InvalidUsername
                    | RenameDocumentError::ExpiredAuth
                    | RenameDocumentError::NotPermissioned
                    | RenameDocumentError::UserNotFound
                    | RenameDocumentError::DocumentNotFound
                    | RenameDocumentError::DocumentDeleted
                    | RenameDocumentError::EditConflict
                    | RenameDocumentError::DocumentPathTaken => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::FolderRenameError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    RenameFolderError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    RenameFolderError::InternalError
                    | RenameFolderError::InvalidAuth
                    | RenameFolderError::InvalidUsername
                    | RenameFolderError::ExpiredAuth
                    | RenameFolderError::NotPermissioned
                    | RenameFolderError::UserNotFound
                    | RenameFolderError::FolderNotFound
                    | RenameFolderError::FolderDeleted
                    | RenameFolderError::EditConflict
                    | RenameFolderError::FolderPathTaken => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::DocumentMoveError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    MoveDocumentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    MoveDocumentError::InternalError
                    | MoveDocumentError::InvalidAuth
                    | MoveDocumentError::InvalidUsername
                    | MoveDocumentError::ExpiredAuth
                    | MoveDocumentError::NotPermissioned
                    | MoveDocumentError::UserNotFound
                    | MoveDocumentError::DocumentNotFound
                    | MoveDocumentError::EditConflict
                    | MoveDocumentError::DocumentDeleted
                    | MoveDocumentError::DocumentPathTaken => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::FolderMoveError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    MoveFolderError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    MoveFolderError::InternalError
                    | MoveFolderError::InvalidAuth
                    | MoveFolderError::InvalidUsername
                    | MoveFolderError::ExpiredAuth
                    | MoveFolderError::NotPermissioned
                    | MoveFolderError::UserNotFound
                    | MoveFolderError::FolderNotFound
                    | MoveFolderError::EditConflict
                    | MoveFolderError::FolderDeleted
                    | MoveFolderError::FolderPathTaken => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::DocumentCreateError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    CreateDocumentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    CreateDocumentError::InternalError
                    | CreateDocumentError::InvalidAuth
                    | CreateDocumentError::InvalidUsername
                    | CreateDocumentError::ExpiredAuth
                    | CreateDocumentError::NotPermissioned
                    | CreateDocumentError::UserNotFound
                    | CreateDocumentError::FileIdTaken
                    | CreateDocumentError::DocumentPathTaken
                    | CreateDocumentError::ParentNotFound => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::FolderCreateError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    CreateFolderError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    CreateFolderError::InternalError
                    | CreateFolderError::InvalidAuth
                    | CreateFolderError::InvalidUsername
                    | CreateFolderError::ExpiredAuth
                    | CreateFolderError::NotPermissioned
                    | CreateFolderError::UserNotFound
                    | CreateFolderError::FileIdTaken
                    | CreateFolderError::FolderPathTaken => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::DocumentChangeError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    ChangeDocumentContentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    ChangeDocumentContentError::InternalError
                    | ChangeDocumentContentError::InvalidAuth
                    | ChangeDocumentContentError::InvalidUsername
                    | ChangeDocumentContentError::ExpiredAuth
                    | ChangeDocumentContentError::NotPermissioned
                    | ChangeDocumentContentError::UserNotFound
                    | ChangeDocumentContentError::DocumentNotFound
                    | ChangeDocumentContentError::EditConflict
                    | ChangeDocumentContentError::DocumentDeleted => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::DocumentDeleteError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    DeleteDocumentError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    DeleteDocumentError::InternalError
                    | DeleteDocumentError::InvalidAuth
                    | DeleteDocumentError::InvalidUsername
                    | DeleteDocumentError::ExpiredAuth
                    | DeleteDocumentError::NotPermissioned
                    | DeleteDocumentError::UserNotFound
                    | DeleteDocumentError::DocumentNotFound
                    | DeleteDocumentError::EditConflict
                    | DeleteDocumentError::DocumentDeleted => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::FolderDeleteError(api) => match api {
                Error::SendFailed(_) => Err(ExecuteWorkError::CouldNotReachServer),
                Error::Api(api_err) => match api_err {
                    DeleteFolderError::ClientUpdateRequired => {
                        Err(ExecuteWorkError::ClientUpdateRequired)
                    }
                    DeleteFolderError::InternalError
                    | DeleteFolderError::InvalidAuth
                    | DeleteFolderError::InvalidUsername
                    | DeleteFolderError::ExpiredAuth
                    | DeleteFolderError::NotPermissioned
                    | DeleteFolderError::UserNotFound
                    | DeleteFolderError::FolderNotFound
                    | DeleteFolderError::EditConflict
                    | DeleteFolderError::FolderDeleted => {
                        Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api_err)))
                    }
                },
                Error::Serialize(_) | Error::ReceiveFailed(_) | Error::Deserialize(_) => {
                    Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", api)))
                }
            },
            WorkExecutionError::MetadataRepoError(_)
            | WorkExecutionError::MetadataRepoErrorOpt(_)
            | WorkExecutionError::SaveDocumentError(_)
            | WorkExecutionError::AutoRenameError(_)
            | WorkExecutionError::ResolveConflictByCreatingNewFileError(_)
            | WorkExecutionError::DecryptingOldVersionForMergeError(_)
            | WorkExecutionError::ReadingCurrentVersionError(_)
            | WorkExecutionError::WritingMergedFileError(_)
            | WorkExecutionError::FindingParentsForConflictingFileError(_)
            | WorkExecutionError::LocalChangesRepoError(_) => {
                Err(ExecuteWorkError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum SetLastSyncedError {
    UnexpectedError(String),
}

pub fn set_last_synced(config: &Config, last_sync: u64) -> Result<(), SetLastSyncedError> {
    let db = connect_to_db(&config).map_err(SetLastSyncedError::UnexpectedError)?;

    match DefaultFileMetadataRepo::set_last_synced(&db, last_sync) {
        Ok(_) => Ok(()),
        Err(err) => Err(SetLastSyncedError::UnexpectedError(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize)]
pub enum GetLastSyncedError {
    UnexpectedError(String),
}

pub fn get_last_synced(config: &Config) -> Result<u64, GetLastSyncedError> {
    let db = connect_to_db(&config).map_err(GetLastSyncedError::UnexpectedError)?;

    match DefaultFileMetadataRepo::get_last_updated(&db) {
        Ok(val) => Ok(val),
        Err(err) => match err {
            DbError::SledError(_) | DbError::SerdeError(_) => {
                Err(GetLastSyncedError::UnexpectedError(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize)]
pub enum GetUsageError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
    UnexpectedError(String),
}

pub fn get_usage(config: &Config) -> Result<Vec<FileUsage>, GetUsageError> {
    let db = connect_to_db(&config).map_err(GetUsageError::UnexpectedError)?;

    let acc = DefaultAccountRepo::get_account(&db).map_err(|_| GetUsageError::NoAccount)?;

    DefaultClient::get_usage(&acc.api_url, acc.username.as_str())
        .map(|resp| resp.usages)
        .map_err(|err| match err {
            Error::Api(api::GetUsageError::ClientUpdateRequired) => {
                GetUsageError::ClientUpdateRequired
            }
            Error::SendFailed(_) | Error::ReceiveFailed(_) => GetUsageError::CouldNotReachServer,
            _ => GetUsageError::UnexpectedError(format!("{:#?}", err)),
        })
}
