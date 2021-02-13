#![recursion_limit = "256"]

#[macro_use]
extern crate log;
extern crate reqwest;

use std::env;
use std::path::Path;
use std::str::FromStr;

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use serde::Serialize;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;

use crate::client::{ApiError, ClientImpl};
use crate::model::account::Account;
use crate::model::api::{FileUsage, GetPublicKeyError, NewAccountError};
use crate::model::crypto::DecryptedDocument;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::repo::account_repo::{AccountRepo, AccountRepoError, AccountRepoImpl};
use crate::repo::db_version_repo::DbVersionRepoImpl;
use crate::repo::document_repo::DocumentRepoImpl;
use crate::repo::file_metadata_repo::{
    DbError, FileMetadataRepo, FileMetadataRepoImpl, Filter, FindingChildrenFailed,
    FindingParentsFailed, GetError as FileMetadataRepoError,
};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{
    AccountCreationError, AccountExportError as ASAccountExportError, AccountImportError,
    AccountService, AccountServiceImpl,
};
use crate::service::clock_service::{Clock, ClockImpl};
use crate::service::code_version_service::CodeVersionImpl;
use crate::service::crypto_service::{AESImpl, RSAImpl};
use crate::service::db_state_service::{DbStateService, DbStateServiceImpl, State};
use crate::service::file_compression_service::FileCompressionServiceImpl;
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{
    DocumentRenameError, DocumentUpdateError, FileMoveError, FileService, FileServiceImpl,
    NewFileError, NewFileFromPathError, ReadDocumentError as FSReadDocumentError,
};
use crate::service::sync_service::{
    CalculateWorkError as SSCalculateWorkError, FileSyncService, SyncError, SyncService,
    WorkCalculated, WorkExecutionError,
};
use crate::service::usage_service::{UsageService, UsageServiceImpl};
use crate::service::{db_state_service, file_service, usage_service};
#[allow(unused_imports)] // For one touch backend switching, allow one of these to be unused
use crate::storage::db_provider::{Backend, FileBackend, SledBackend};

#[derive(Debug, Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error<U: Serialize> {
    UiError(U),
    Unexpected(String),
}
use crate::model::drawing::Drawing;
use crate::service::drawing_service::{DrawingError, DrawingService, DrawingServiceImpl};
use serde_json::error::Category;
use Error::UiError;

macro_rules! unexpected {
    ($base:literal $(, $args:tt )*) => {
        Error::Unexpected(format!($base $(, $args )*))
    };
}

macro_rules! connect_to_db {
    ($cfg:expr) => {
        DefaultBackend::connect_to_db($cfg).map_err(|err| unexpected!("{:#?}", err))
    };
}

pub fn init_logger(log_path: &Path) -> Result<(), Error<()>> {
    let print_colors = env::var("LOG_NO_COLOR").is_err();
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| log::LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or(log::LevelFilter::Debug);

    loggers::init(log_path, LOG_FILE.to_string(), print_colors)
        .map_err(|err| unexpected!("IO Error: {:#?}", err))?
        .level(log::LevelFilter::Warn)
        .level_for("lockbook_core", lockbook_log_level)
        .apply()
        .map_err(|err| unexpected!("{:#?}", err))?;
    info!("Logger initialized! Path: {:?}", log_path);
    Ok(())
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetStateError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_db_state(config: &Config) -> Result<State, Error<GetStateError>> {
    let backend = connect_to_db!(config)?;
    DefaultDbStateService::get_state(&backend).map_err(|err| unexpected!("{:#?}", err))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MigrationError {
    StateRequiresCleaning,
}

pub fn migrate_db(config: &Config) -> Result<(), Error<MigrationError>> {
    let backend = connect_to_db!(config)?;

    DefaultDbStateService::perform_migration(&backend).map_err(|e| match e {
        db_state_service::MigrationError::StateRequiresClearing => {
            UiError(MigrationError::StateRequiresCleaning)
        }
        db_state_service::MigrationError::RepoError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateAccountError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
}

pub fn create_account(
    config: &Config,
    username: &str,
    api_url: &str,
) -> Result<Account, Error<CreateAccountError>> {
    let backend = connect_to_db!(config)?;

    DefaultAccountService::create_account(&backend, username, api_url).map_err(|e| match e {
        AccountCreationError::AccountExistsAlready => {
            UiError(CreateAccountError::AccountExistsAlready)
        }
        AccountCreationError::ApiError(network) => match network {
            ApiError::Endpoint(api_err) => match api_err {
                NewAccountError::UsernameTaken => UiError(CreateAccountError::UsernameTaken),
                NewAccountError::InvalidUsername => UiError(CreateAccountError::InvalidUsername),
                NewAccountError::InvalidPublicKey
                | NewAccountError::InvalidUserAccessKey
                | NewAccountError::FileIdTaken => unexpected!("{:#?}", api_err),
            },
            ApiError::SendFailed(_) => UiError(CreateAccountError::CouldNotReachServer),
            ApiError::ClientUpdateRequired => UiError(CreateAccountError::ClientUpdateRequired),
            ApiError::Serialize(_)
            | ApiError::ReceiveFailed(_)
            | ApiError::Deserialize(_)
            | ApiError::Sign(_)
            | ApiError::InternalError
            | ApiError::BadRequest
            | ApiError::InvalidAuth
            | ApiError::ExpiredAuth => unexpected!("{:#?}", network),
        },
        AccountCreationError::KeyGenerationError(_)
        | AccountCreationError::AccountRepoError(_)
        | AccountCreationError::FolderError(_)
        | AccountCreationError::MetadataRepoError(_)
        | AccountCreationError::KeySerializationError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ImportError {
    AccountStringCorrupted,
    AccountExistsAlready,
    AccountDoesNotExist,
    UsernamePKMismatch,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn import_account(
    config: &Config,
    account_string: &str,
) -> Result<Account, Error<ImportError>> {
    let backend = connect_to_db!(config)?;

    DefaultAccountService::import_account(&backend, account_string).map_err(|e| match e {
        AccountImportError::AccountStringCorrupted(_)
        | AccountImportError::AccountStringFailedToDeserialize(_)
        | AccountImportError::InvalidPrivateKey(_) => UiError(ImportError::AccountStringCorrupted),
        AccountImportError::AccountExistsAlready => UiError(ImportError::AccountExistsAlready),
        AccountImportError::PublicKeyMismatch => UiError(ImportError::UsernamePKMismatch),
        AccountImportError::FailedToVerifyAccountServerSide(client_err) => match client_err {
            ApiError::SendFailed(_) => UiError(ImportError::CouldNotReachServer),
            ApiError::Endpoint(api_err) => match api_err {
                GetPublicKeyError::UserNotFound => UiError(ImportError::AccountDoesNotExist),
                GetPublicKeyError::InvalidUsername => unexpected!("{:#?}", api_err),
            },
            ApiError::ClientUpdateRequired => UiError(ImportError::ClientUpdateRequired),
            ApiError::Serialize(_)
            | ApiError::ReceiveFailed(_)
            | ApiError::Deserialize(_)
            | ApiError::Sign(_)
            | ApiError::InternalError
            | ApiError::BadRequest
            | ApiError::InvalidAuth
            | ApiError::ExpiredAuth => unexpected!("{:#?}", client_err),
        },
        AccountImportError::PersistenceError(_) | AccountImportError::AccountRepoError(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AccountExportError {
    NoAccount,
}

pub fn export_account(config: &Config) -> Result<String, Error<AccountExportError>> {
    let backend = connect_to_db!(config)?;

    DefaultAccountService::export_account(&backend).map_err(|e| match e {
        ASAccountExportError::AccountRetrievalError(db_err) => match db_err {
            AccountRepoError::NoAccount => UiError(AccountExportError::NoAccount),
            AccountRepoError::SerdeError(_) | AccountRepoError::BackendError(_) => {
                unexpected!("{:#?}", db_err)
            }
        },
        ASAccountExportError::AccountStringFailedToSerialize(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

pub fn get_account(config: &Config) -> Result<Account, Error<GetAccountError>> {
    let backend = connect_to_db!(config)?;

    DefaultAccountRepo::get_account(&backend).map_err(|e| match e {
        AccountRepoError::NoAccount => UiError(GetAccountError::NoAccount),
        AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateFileAtPathError {
    FileAlreadyExists,
    NoAccount,
    NoRoot,
    PathDoesntStartWithRoot,
    PathContainsEmptyFile,
    DocumentTreatedAsFolder,
}

pub fn create_file_at_path(
    config: &Config,
    path_and_name: &str,
) -> Result<FileMetadata, Error<CreateFileAtPathError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::create_at_path(&backend, path_and_name).map_err(|e| match e {
        NewFileFromPathError::PathDoesntStartWithRoot => {
            UiError(CreateFileAtPathError::PathDoesntStartWithRoot)
        }
        NewFileFromPathError::PathContainsEmptyFile => {
            UiError(CreateFileAtPathError::PathContainsEmptyFile)
        }
        NewFileFromPathError::FileAlreadyExists => {
            UiError(CreateFileAtPathError::FileAlreadyExists)
        }
        NewFileFromPathError::NoRoot => UiError(CreateFileAtPathError::NoRoot),
        NewFileFromPathError::FailedToCreateChild(failed_to_create) => match failed_to_create {
            NewFileError::AccountRetrievalError(account_error) => match account_error {
                AccountRepoError::NoAccount => UiError(CreateFileAtPathError::NoAccount),
                AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                    unexpected!("{:#?}", account_error)
                }
            },
            NewFileError::FileNameNotAvailable => UiError(CreateFileAtPathError::FileAlreadyExists),
            NewFileError::DocumentTreatedAsFolder => {
                UiError(CreateFileAtPathError::DocumentTreatedAsFolder)
            }
            NewFileError::CouldNotFindParents(_)
            | NewFileError::FileCryptoError(_)
            | NewFileError::MetadataRepoError(_)
            | NewFileError::FailedToWriteFileContent(_)
            | NewFileError::FailedToRecordChange(_)
            | NewFileError::FileNameEmpty
            | NewFileError::FileNameContainsSlash => unexpected!("{:#?}", failed_to_create),
        },
        NewFileFromPathError::FailedToRecordChange(_) | NewFileFromPathError::DbError(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum WriteToDocumentError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDocument,
}

pub fn write_document(
    config: &Config,
    id: Uuid,
    content: &[u8],
) -> Result<(), Error<WriteToDocumentError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::write_document(&backend, id, content).map_err(|e| match e {
        DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
            AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                unexpected!("{:#?}", account_err)
            }
            AccountRepoError::NoAccount => UiError(WriteToDocumentError::NoAccount),
        },
        DocumentUpdateError::CouldNotFindFile => UiError(WriteToDocumentError::FileDoesNotExist),
        DocumentUpdateError::FolderTreatedAsDocument => {
            UiError(WriteToDocumentError::FolderTreatedAsDocument)
        }
        DocumentUpdateError::CouldNotFindParents(_)
        | DocumentUpdateError::FileCryptoError(_)
        | DocumentUpdateError::FileCompressionError(_)
        | DocumentUpdateError::FileDecompressionError(_)
        | DocumentUpdateError::DocumentWriteError(_)
        | DocumentUpdateError::DbError(_)
        | DocumentUpdateError::FetchOldVersionError(_)
        | DocumentUpdateError::DecryptOldVersionError(_)
        | DocumentUpdateError::AccessInfoCreationError(_)
        | DocumentUpdateError::FailedToRecordChange(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateFileError {
    NoAccount,
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameEmpty,
    FileNameContainsSlash,
}

pub fn create_file(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<FileMetadata, Error<CreateFileError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::create(&backend, name, parent, file_type).map_err(|e| match e {
        NewFileError::AccountRetrievalError(_) => UiError(CreateFileError::NoAccount),
        NewFileError::DocumentTreatedAsFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
        NewFileError::CouldNotFindParents(parent_error) => match parent_error {
            FindingParentsFailed::AncestorMissing => UiError(CreateFileError::CouldNotFindAParent),
            FindingParentsFailed::DbError(_) => unexpected!("{:#?}", parent_error),
        },
        NewFileError::FileNameNotAvailable => UiError(CreateFileError::FileNameNotAvailable),
        NewFileError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
        NewFileError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
        NewFileError::FileCryptoError(_)
        | NewFileError::MetadataRepoError(_)
        | NewFileError::FailedToWriteFileContent(_)
        | NewFileError::FailedToRecordChange(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

pub fn get_root(config: &Config) -> Result<FileMetadata, Error<GetRootError>> {
    let backend = connect_to_db!(config)?;

    match DefaultFileMetadataRepo::get_root(&backend) {
        Ok(file_metadata) => match file_metadata {
            None => Err(UiError(GetRootError::NoRoot)),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(unexpected!("{:#?}", err)),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetChildrenError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_children(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, Error<GetChildrenError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get_children_non_recursively(&backend, id)
        .map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAndGetChildrenError {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
}

pub fn get_and_get_children_recursively(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, Error<GetAndGetChildrenError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get_and_get_children_recursively(&backend, id).map_err(|e| match e {
        FindingChildrenFailed::FileDoesNotExist => {
            UiError(GetAndGetChildrenError::FileDoesNotExist)
        }
        FindingChildrenFailed::DocumentTreatedAsFolder => {
            UiError(GetAndGetChildrenError::DocumentTreatedAsFolder)
        }
        FindingChildrenFailed::DbError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByIdError {
    NoFileWithThatId,
}

pub fn get_file_by_id(config: &Config, id: Uuid) -> Result<FileMetadata, Error<GetFileByIdError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get(&backend, id).map_err(|e| match e {
        FileMetadataRepoError::FileRowMissing => UiError(GetFileByIdError::NoFileWithThatId),
        FileMetadataRepoError::DbError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

pub fn get_file_by_path(
    config: &Config,
    path: &str,
) -> Result<FileMetadata, Error<GetFileByPathError>> {
    let backend = connect_to_db!(config)?;

    match DefaultFileMetadataRepo::get_by_path(&backend, path) {
        Ok(maybe_file_metadata) => match maybe_file_metadata {
            None => Err(UiError(GetFileByPathError::NoFileAtThatPath)),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(unexpected!("{:#?}", err)),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum InsertFileError {
    Stub, // TODO: Enums should not be empty
}

pub fn insert_file(
    config: &Config,
    file_metadata: FileMetadata,
) -> Result<(), Error<InsertFileError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::insert(&backend, &file_metadata).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), Error<FileDeleteError>> {
    let backend = connect_to_db!(config)?;

    match DefaultFileMetadataRepo::get(&backend, id) {
        Ok(meta) => match meta.file_type {
            FileType::Document => {
                DefaultFileService::delete_document(&backend, id).map_err(|err| match err {
                    file_service::DeleteDocumentError::CouldNotFindFile
                    | file_service::DeleteDocumentError::FolderTreatedAsDocument
                    | file_service::DeleteDocumentError::FailedToRecordChange(_)
                    | file_service::DeleteDocumentError::FailedToUpdateMetadata(_)
                    | file_service::DeleteDocumentError::FailedToDeleteDocument(_)
                    | file_service::DeleteDocumentError::FailedToTrackDelete(_)
                    | file_service::DeleteDocumentError::DbError(_) => {
                        unexpected!("{:#?}", err)
                    }
                })
            }
            FileType::Folder => {
                DefaultFileService::delete_folder(&backend, id).map_err(|err| match err {
                    file_service::DeleteFolderError::CannotDeleteRoot => {
                        UiError(FileDeleteError::CannotDeleteRoot)
                    }
                    file_service::DeleteFolderError::MetadataError(_)
                    | file_service::DeleteFolderError::CouldNotFindFile
                    | file_service::DeleteFolderError::FailedToDeleteMetadata(_)
                    | file_service::DeleteFolderError::FindingChildrenFailed(_)
                    | file_service::DeleteFolderError::FailedToRecordChange(_)
                    | file_service::DeleteFolderError::CouldNotFindParents(_)
                    | file_service::DeleteFolderError::DocumentTreatedAsFolder
                    | file_service::DeleteFolderError::FailedToDeleteDocument(_)
                    | file_service::DeleteFolderError::FailedToDeleteChangeEntry(_) => {
                        unexpected!("{:#?}", err)
                    }
                })
            }
        },
        Err(_) => Err(UiError(FileDeleteError::FileDoesNotExist)),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ReadDocumentError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
}

pub fn read_document(
    config: &Config,
    id: Uuid,
) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::read_document(&backend, id).map_err(|e| match e {
        FSReadDocumentError::TreatedFolderAsDocument => {
            UiError(ReadDocumentError::TreatedFolderAsDocument)
        }

        FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
            AccountRepoError::NoAccount => UiError(ReadDocumentError::NoAccount),
            AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                unexpected!("{:#?}", account_error)
            }
        },
        FSReadDocumentError::CouldNotFindFile => UiError(ReadDocumentError::FileDoesNotExist),
        FSReadDocumentError::DbError(_)
        | FSReadDocumentError::DocumentReadError(_)
        | FSReadDocumentError::CouldNotFindParents(_)
        | FSReadDocumentError::FileCryptoError(_)
        | FSReadDocumentError::FileDecompressionError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListPathsError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_paths(
    config: &Config,
    filter: Option<Filter>,
) -> Result<Vec<String>, Error<ListPathsError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get_all_paths(&backend, filter).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListMetadatasError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_metadatas(config: &Config) -> Result<Vec<FileMetadata>, Error<ListMetadatasError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get_all(&backend).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum RenameFileError {
    FileDoesNotExist,
    NewNameEmpty,
    NewNameContainsSlash,
    FileNameNotAvailable,
    CannotRenameRoot,
}

pub fn rename_file(
    config: &Config,
    id: Uuid,
    new_name: &str,
) -> Result<(), Error<RenameFileError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::rename_file(&backend, id, new_name).map_err(|e| match e {
        DocumentRenameError::FileDoesNotExist => UiError(RenameFileError::FileDoesNotExist),
        DocumentRenameError::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
        DocumentRenameError::FileNameContainsSlash => {
            UiError(RenameFileError::NewNameContainsSlash)
        }
        DocumentRenameError::FileNameNotAvailable => UiError(RenameFileError::FileNameNotAvailable),
        DocumentRenameError::CannotRenameRoot => UiError(RenameFileError::CannotRenameRoot),
        DocumentRenameError::DbError(_) | DocumentRenameError::FailedToRecordChange(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MoveFileError {
    CannotMoveRoot,
    DocumentTreatedAsFolder,
    FileDoesNotExist,
    FolderMovedIntoItself,
    NoAccount,
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
}

pub fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileService::move_file(&backend, id, new_parent).map_err(|e| match e {
        FileMoveError::DocumentTreatedAsFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
        FileMoveError::FolderMovedIntoItself => UiError(MoveFileError::FolderMovedIntoItself),
        FileMoveError::AccountRetrievalError(account_err) => match account_err {
            AccountRepoError::NoAccount => UiError(MoveFileError::NoAccount),
            AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                unexpected!("{:#?}", account_err)
            }
        },
        FileMoveError::TargetParentHasChildNamedThat => {
            UiError(MoveFileError::TargetParentHasChildNamedThat)
        }
        FileMoveError::FileDoesNotExist => UiError(MoveFileError::FileDoesNotExist),
        FileMoveError::TargetParentDoesNotExist => UiError(MoveFileError::TargetParentDoesNotExist),
        FileMoveError::CannotMoveRoot => UiError(MoveFileError::CannotMoveRoot),
        FileMoveError::DbError(_)
        | FileMoveError::FindingChildrenFailed(_)
        | FileMoveError::FailedToRecordChange(_)
        | FileMoveError::FailedToDecryptKey(_)
        | FileMoveError::FailedToReEncryptKey(_)
        | FileMoveError::CouldNotFindParents(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SyncAllError {
    NoAccount,
    ClientUpdateRequired,
    CouldNotReachServer,
    ExecuteWorkError, // TODO: @parth ExecuteWorkError(Vec<Error<ExecuteWorkError>>),
}

pub fn sync_all(config: &Config) -> Result<(), Error<SyncAllError>> {
    let backend = connect_to_db!(config)?;

    DefaultSyncService::sync(&backend).map_err(|e| match e {
        SyncError::AccountRetrievalError(err) => match err {
            AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                unexpected!("{:#?}", err)
            }
            AccountRepoError::NoAccount => UiError(SyncAllError::NoAccount),
        },
        SyncError::CalculateWorkError(err) => match err {
            SSCalculateWorkError::LocalChangesRepoError(_)
            | SSCalculateWorkError::MetadataRepoError(_)
            | SSCalculateWorkError::GetMetadataError(_) => unexpected!("{:#?}", err),
            SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::NoAccount => UiError(SyncAllError::NoAccount),
                AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                    unexpected!("{:#?}", account_err)
                }
            },
            SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
                ApiError::SendFailed(_) => UiError(SyncAllError::CouldNotReachServer),
                ApiError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
                ApiError::Serialize(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_)
                | ApiError::Sign(_)
                | ApiError::InternalError
                | ApiError::BadRequest
                | ApiError::InvalidAuth
                | ApiError::ExpiredAuth
                | ApiError::Endpoint(_) => unexpected!("{:#?}", api_err),
            },
        },
        SyncError::WorkExecutionError(_) => UiError(SyncAllError::ExecuteWorkError),
        SyncError::MetadataUpdateError(err) => unexpected!("{:#?}", err),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, Error<CalculateWorkError>> {
    let backend = connect_to_db!(config)?;

    DefaultSyncService::calculate_work(&backend).map_err(|e| match e {
        SSCalculateWorkError::LocalChangesRepoError(_)
        | SSCalculateWorkError::MetadataRepoError(_)
        | SSCalculateWorkError::GetMetadataError(_) => unexpected!("{:#?}", e),
        SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
            AccountRepoError::NoAccount => UiError(CalculateWorkError::NoAccount),
            AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                unexpected!("{:#?}", account_err)
            }
        },
        SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
            ApiError::SendFailed(_) => UiError(CalculateWorkError::CouldNotReachServer),
            ApiError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
            ApiError::Serialize(_)
            | ApiError::ReceiveFailed(_)
            | ApiError::Deserialize(_)
            | ApiError::Sign(_)
            | ApiError::InternalError
            | ApiError::BadRequest
            | ApiError::InvalidAuth
            | ApiError::ExpiredAuth
            | ApiError::Endpoint(_) => unexpected!("{:#?}", api_err),
        },
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExecuteWorkError {
    CouldNotReachServer,
    ClientUpdateRequired,
    BadAccount, // FIXME: @raayan Temporary to avoid passing key through FFI
}

pub fn execute_work(
    config: &Config,
    account: &Account,
    wu: WorkUnit,
) -> Result<(), Error<ExecuteWorkError>> {
    let backend = connect_to_db!(config)?;

    DefaultSyncService::execute_work(&backend, &account, wu).map_err(|e| match e {
        WorkExecutionError::SendFailed(_) => UiError(ExecuteWorkError::CouldNotReachServer),
        WorkExecutionError::ClientUpdateRequired => UiError(ExecuteWorkError::ClientUpdateRequired),
        WorkExecutionError::MetadataRepoError(_)
        | WorkExecutionError::MetadataRepoErrorOpt(_)
        | WorkExecutionError::SaveDocumentError(_)
        | WorkExecutionError::AutoRenameError(_)
        | WorkExecutionError::ResolveConflictByCreatingNewFileError(_)
        | WorkExecutionError::DecryptingOldVersionForMergeError(_)
        | WorkExecutionError::DecompressingForMergeError(_)
        | WorkExecutionError::ReadingCurrentVersionError(_)
        | WorkExecutionError::WritingMergedFileError(_)
        | WorkExecutionError::ErrorCreatingRecoveryFile(_)
        | WorkExecutionError::ErrorCalculatingCurrentTime(_)
        | WorkExecutionError::FindingParentsForConflictingFileError(_)
        | WorkExecutionError::LocalFolderDeleteError(_)
        | WorkExecutionError::FindingChildrenFailed(_)
        | WorkExecutionError::RecursiveDeleteError(_)
        | WorkExecutionError::LocalChangesRepoError(_)
        | WorkExecutionError::InvalidAuth
        | WorkExecutionError::ExpiredAuth
        | WorkExecutionError::InternalError
        | WorkExecutionError::BadRequest
        | WorkExecutionError::Sign(_)
        | WorkExecutionError::Serialize(_)
        | WorkExecutionError::ReceiveFailed(_)
        | WorkExecutionError::Deserialize(_)
        | WorkExecutionError::DocumentGetError(_)
        | WorkExecutionError::DocumentRenameError(_)
        | WorkExecutionError::FolderRenameError(_)
        | WorkExecutionError::DocumentMoveError(_)
        | WorkExecutionError::FolderMoveError(_)
        | WorkExecutionError::DocumentCreateError(_)
        | WorkExecutionError::FolderCreateError(_)
        | WorkExecutionError::DocumentChangeError(_)
        | WorkExecutionError::DocumentDeleteError(_)
        | WorkExecutionError::FolderDeleteError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn set_last_synced(config: &Config, last_sync: u64) -> Result<(), Error<SetLastSyncedError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::set_last_synced(&backend, last_sync)
        .map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_last_synced(config: &Config) -> Result<i64, Error<GetLastSyncedError>> {
    let backend = connect_to_db!(config)?;

    DefaultFileMetadataRepo::get_last_updated(&backend)
        .map(|n| n as i64)
        .map_err(|err| match err {
            DbError::BackendError(_) | DbError::SerdeError(_) => unexpected!("{:#?}", err),
        })
}

pub fn get_last_synced_human_string(config: &Config) -> Result<String, Error<GetLastSyncedError>> {
    let last_synced = get_last_synced(config)?;

    Ok(if last_synced != 0 {
        Duration::milliseconds(DefaultClock::get_time() - last_synced)
            .format_human()
            .to_string()
    } else {
        "never".to_string()
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetUsageError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn get_usage(config: &Config) -> Result<Vec<FileUsage>, Error<GetUsageError>> {
    let backend = connect_to_db!(config)?;

    DefaultUsageService::get_usage(&backend)
        .map(|resp| resp.usages)
        .map_err(|err| match err {
            usage_service::GetUsageError::AccountRetrievalError(db_err) => match db_err {
                AccountRepoError::NoAccount => UiError(GetUsageError::NoAccount),
                AccountRepoError::SerdeError(_) | AccountRepoError::BackendError(_) => {
                    unexpected!("{:#?}", db_err)
                }
            },
            usage_service::GetUsageError::ApiError(api_err) => match api_err {
                ApiError::SendFailed(_) => UiError(GetUsageError::CouldNotReachServer),
                ApiError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
                ApiError::Endpoint(_)
                | ApiError::InvalidAuth
                | ApiError::ExpiredAuth
                | ApiError::InternalError
                | ApiError::BadRequest
                | ApiError::Sign(_)
                | ApiError::Serialize(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_) => unexpected!("{:#?}", api_err),
            },
        })
}

pub fn get_usage_human_string(
    config: &Config,
    exact: bool,
) -> Result<String, Error<GetUsageError>> {
    let backend = connect_to_db!(config)?;

    DefaultUsageService::get_usage_human_string(&backend, exact).map_err(|err| match err {
        usage_service::GetUsageError::AccountRetrievalError(db_err) => match db_err {
            AccountRepoError::NoAccount => UiError(GetUsageError::NoAccount),
            AccountRepoError::SerdeError(_) | AccountRepoError::BackendError(_) => {
                unexpected!("{:#?}", db_err)
            }
        },
        usage_service::GetUsageError::ApiError(api_err) => match api_err {
            ApiError::SendFailed(_) => UiError(GetUsageError::CouldNotReachServer),
            ApiError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
            ApiError::Endpoint(_)
            | ApiError::InvalidAuth
            | ApiError::ExpiredAuth
            | ApiError::InternalError
            | ApiError::BadRequest
            | ApiError::Sign(_)
            | ApiError::Serialize(_)
            | ApiError::ReceiveFailed(_)
            | ApiError::Deserialize(_) => unexpected!("{:#?}", api_err),
        },
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetDrawingError {
    TreatedFolderAsDrawing,
    NoAccount,
    FileDoesNotExist,
}

pub fn get_drawing(config: &Config, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
    let backend = connect_to_db!(config)?;

    DefaultDrawingService::get_drawing(&backend, id).map_err(|drawing_err| match drawing_err {
        DrawingError::InvalidDrawingError(err) => unexpected!("{:#?}", err),
        DrawingError::FailedToSaveDrawing(err) => match err {
            DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                    unexpected!("{:#?}", account_err)
                }
                AccountRepoError::NoAccount => UiError(GetDrawingError::NoAccount),
            },
            DocumentUpdateError::CouldNotFindFile => UiError(GetDrawingError::FileDoesNotExist),
            DocumentUpdateError::FolderTreatedAsDocument => {
                UiError(GetDrawingError::TreatedFolderAsDrawing)
            }
            DocumentUpdateError::CouldNotFindParents(_)
            | DocumentUpdateError::FileCryptoError(_)
            | DocumentUpdateError::FileCompressionError(_)
            | DocumentUpdateError::FileDecompressionError(_)
            | DocumentUpdateError::DocumentWriteError(_)
            | DocumentUpdateError::DbError(_)
            | DocumentUpdateError::FetchOldVersionError(_)
            | DocumentUpdateError::DecryptOldVersionError(_)
            | DocumentUpdateError::AccessInfoCreationError(_)
            | DocumentUpdateError::FailedToRecordChange(_) => unexpected!("{:#?}", err),
        },
        DrawingError::FailedToRetrieveDrawing(err) => match err {
            FSReadDocumentError::TreatedFolderAsDocument => {
                UiError(GetDrawingError::TreatedFolderAsDrawing)
            }
            FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                AccountRepoError::NoAccount => UiError(GetDrawingError::NoAccount),
                AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                    unexpected!("{:#?}", account_error)
                }
            },
            FSReadDocumentError::CouldNotFindFile => UiError(GetDrawingError::FileDoesNotExist),
            FSReadDocumentError::DbError(_)
            | FSReadDocumentError::DocumentReadError(_)
            | FSReadDocumentError::CouldNotFindParents(_)
            | FSReadDocumentError::FileCryptoError(_)
            | FSReadDocumentError::FileDecompressionError(_) => unexpected!("{:#?}", err),
        },
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDrawingError {
    InvalidDrawing,
    NoAccount,
    FileDoesNotExist,
    TreatedFolderAsDrawing,
}

pub fn save_drawing(
    config: &Config,
    id: Uuid,
    serialized_drawing: &str,
) -> Result<(), Error<SaveDrawingError>> {
    let backend = connect_to_db!(config)?;

    DefaultDrawingService::save_drawing(&backend, id, serialized_drawing).map_err(|drawing_err| {
        match drawing_err {
            DrawingError::InvalidDrawingError(err) => match err.classify() {
                Category::Io => unexpected!("{:#?}", err),
                Category::Syntax | Category::Data | Category::Eof => {
                    UiError(SaveDrawingError::InvalidDrawing)
                }
            },
            DrawingError::FailedToSaveDrawing(err) => match err {
                DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
                    AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                        unexpected!("{:#?}", account_err)
                    }
                    AccountRepoError::NoAccount => UiError(SaveDrawingError::NoAccount),
                },
                DocumentUpdateError::CouldNotFindFile => {
                    UiError(SaveDrawingError::FileDoesNotExist)
                }
                DocumentUpdateError::FolderTreatedAsDocument => {
                    UiError(SaveDrawingError::TreatedFolderAsDrawing)
                }
                DocumentUpdateError::CouldNotFindParents(_)
                | DocumentUpdateError::FileCryptoError(_)
                | DocumentUpdateError::FileCompressionError(_)
                | DocumentUpdateError::FileDecompressionError(_)
                | DocumentUpdateError::DocumentWriteError(_)
                | DocumentUpdateError::DbError(_)
                | DocumentUpdateError::FetchOldVersionError(_)
                | DocumentUpdateError::DecryptOldVersionError(_)
                | DocumentUpdateError::AccessInfoCreationError(_)
                | DocumentUpdateError::FailedToRecordChange(_) => unexpected!("{:#?}", err),
            },
            DrawingError::FailedToRetrieveDrawing(err) => match err {
                FSReadDocumentError::TreatedFolderAsDocument => {
                    UiError(SaveDrawingError::TreatedFolderAsDrawing)
                }
                FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount => UiError(SaveDrawingError::NoAccount),
                    AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                        unexpected!("{:#?}", account_error)
                    }
                },
                FSReadDocumentError::CouldNotFindFile => {
                    UiError(SaveDrawingError::FileDoesNotExist)
                }
                FSReadDocumentError::DbError(_)
                | FSReadDocumentError::DocumentReadError(_)
                | FSReadDocumentError::CouldNotFindParents(_)
                | FSReadDocumentError::FileCryptoError(_)
                | FSReadDocumentError::FileDecompressionError(_) => unexpected!("{:#?}", err),
            },
        }
    })
}

// This basically generates a function called `get_all_error_variants`,
// which will produce a big json dict of { "Error": ["Values"] }.
// Clients can consume this and attempt deserializing each array of errors to see
// if they are handling all cases
macro_rules! impl_get_variants {
    ( $( $name:ty,)* ) => {
        fn get_all_error_variants() -> Value {
            json!({
                $(stringify!($name): <$name>::iter().collect::<Vec<_>>(),)*
            })
        }
    };
}

// All new errors must be placed in here!
impl_get_variants!(
    GetStateError,
    MigrationError,
    CreateAccountError,
    ImportError,
    AccountExportError,
    GetAccountError,
    CreateFileAtPathError,
    WriteToDocumentError,
    CreateFileError,
    GetRootError,
    GetChildrenError,
    GetFileByIdError,
    GetFileByPathError,
    InsertFileError,
    FileDeleteError,
    ReadDocumentError,
    ListPathsError,
    ListMetadatasError,
    RenameFileError,
    MoveFileError,
    SyncAllError,
    CalculateWorkError,
    ExecuteWorkError,
    SetLastSyncedError,
    GetLastSyncedError,
    GetUsageError,
);

pub mod c_interface;
pub mod client;
pub mod java_interface;
mod json_interface;
pub mod loggers;
pub mod model;
pub mod repo;
pub mod service;
pub mod storage;

pub static DEFAULT_API_LOCATION: &str = "http://api.lockbook.app:8000";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
static DB_NAME: &str = "lockbook.sled";
static LOG_FILE: &str = "lockbook.log";

pub type DefaultClock = ClockImpl;
pub type DefaultCrypto = RSAImpl<DefaultClock>;
pub type DefaultSymmetric = AESImpl;
pub type DefaultBackend = FileBackend;
pub type DefaultCodeVersion = CodeVersionImpl;
pub type DefaultClient = ClientImpl<DefaultCrypto, DefaultCodeVersion>;
pub type DefaultAccountRepo = AccountRepoImpl<DefaultBackend>;
pub type DefaultUsageService = UsageServiceImpl<DefaultBackend, DefaultAccountRepo, DefaultClient>;
pub type DefaultDrawingService = DrawingServiceImpl<DefaultBackend, DefaultFileService>;
pub type DefaultDbVersionRepo = DbVersionRepoImpl<DefaultBackend>;
pub type DefaultDbStateService = DbStateServiceImpl<
    DefaultAccountRepo,
    DefaultDbVersionRepo,
    DefaultCodeVersion,
    DefaultBackend,
>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
    DefaultBackend,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl<DefaultBackend>;
pub type DefaultLocalChangesRepo = LocalChangesRepoImpl<DefaultClock, DefaultBackend>;
pub type DefaultDocumentRepo = DocumentRepoImpl<DefaultBackend>;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultFileCompressionService = FileCompressionServiceImpl;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultLocalChangesRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultFileService,
    DefaultFileEncryptionService,
    DefaultFileCompressionService,
    DefaultBackend,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultLocalChangesRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
    DefaultFileCompressionService,
    DefaultBackend,
>;
