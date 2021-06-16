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

use crate::client::ApiError;
use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoError;
use crate::repo::file_metadata_repo::{
    DbError, Filter, FindingChildrenFailed, FindingParentsFailed, GetError as FileMetadataRepoError,
};
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::account_service::{
    AccountCreationError, AccountExportError as ASAccountExportError, AccountImportError,
};
use crate::service::db_state_service::State;
use crate::service::drawing_service::{
    ExportDrawingError as FSExportDrawingError,
    ExportDrawingToDiskError as FSExportDrawingToDiskError, GetDrawingError as FSGetDrawingError,
    SaveDrawingError as FSSaveDrawingError,
};
use crate::service::file_service::{
    DocumentRenameError, DocumentUpdateError, FileMoveError, NewFileError, NewFileFromPathError,
    ReadDocumentError as FSReadDocumentError, SaveDocumentToDiskError as FSSaveDocumentToDiskError,
};
use crate::service::sync_service::{
    CalculateWorkError as SSCalculateWorkError, SyncError, SyncProgress, WorkCalculated,
};
use crate::service::usage_service::{
    GetUsageError as USGetUsageError, LocalAndServerUsageError as USLocalAndServerUsageError,
    LocalAndServerUsages,
};
use crate::service::{
    account_service, db_state_service, drawing_service, file_service, sync_service, usage_service,
};
use lockbook_crypto::clock_service;
use lockbook_models::account::Account;
use lockbook_models::api::{FileUsage, GetPublicKeyError, NewAccountError};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{FileMetadata, FileType};

#[derive(Debug, Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error<U: Serialize> {
    UiError(U),
    Unexpected(String),
}
use crate::repo::local_changes_repo;
use crate::service::drawing_service::SupportedImageFormats;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use serde_json::error::Category;
use std::collections::HashMap;
use std::io::ErrorKind;
use Error::UiError;

macro_rules! unexpected {
    ($base:literal $(, $args:tt )*) => {
        Error::Unexpected(format!($base $(, $args )*))
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
    db_state_service::get_state(&config).map_err(|err| unexpected!("{:#?}", err))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MigrationError {
    StateRequiresCleaning,
}

pub fn migrate_db(config: &Config) -> Result<(), Error<MigrationError>> {
    db_state_service::perform_migration(&config).map_err(|e| match e {
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
    account_service::create_account(&config, username, api_url).map_err(|e| match e {
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
        AccountCreationError::AccountRepoError(_)
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
    account_service::import_account(&config, account_string).map_err(|e| match e {
        AccountImportError::AccountStringCorrupted(_)
        | AccountImportError::AccountStringFailedToDeserialize(_) => {
            UiError(ImportError::AccountStringCorrupted)
        }
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
    account_service::export_account(&config).map_err(|e| match e {
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
    account_repo::get_account(&config).map_err(|e| match e {
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
    file_service::create_at_path(&config, path_and_name).map_err(|e| match e {
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
            | NewFileError::FileEncryptionError(_)
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
    file_service::write_document(&config, id, content).map_err(|e| match e {
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
        | DocumentUpdateError::FileEncryptionError(_)
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
    file_service::create(&config, name, parent, file_type).map_err(|e| match e {
        NewFileError::AccountRetrievalError(_) => UiError(CreateFileError::NoAccount),
        NewFileError::DocumentTreatedAsFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
        NewFileError::CouldNotFindParents(parent_error) => match parent_error {
            FindingParentsFailed::AncestorMissing => UiError(CreateFileError::CouldNotFindAParent),
            FindingParentsFailed::DbError(_) => unexpected!("{:#?}", parent_error),
        },
        NewFileError::FileNameNotAvailable => UiError(CreateFileError::FileNameNotAvailable),
        NewFileError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
        NewFileError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
        NewFileError::FileEncryptionError(_)
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
    match file_metadata_repo::get_root(&config) {
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
    file_metadata_repo::get_children_non_recursively(&config, id)
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
    file_metadata_repo::get_and_get_children_recursively(&config, id).map_err(|e| match e {
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
    file_metadata_repo::get(&config, id).map_err(|e| match e {
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
    match file_metadata_repo::get_by_path(&config, path) {
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
    file_metadata_repo::insert(&config, &file_metadata).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), Error<FileDeleteError>> {
    match file_metadata_repo::get(&config, id) {
        Ok(meta) => match meta.file_type {
            FileType::Document => {
                file_service::delete_document(&config, id).map_err(|err| match err {
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
            FileType::Folder => file_service::delete_folder(&config, id).map_err(|err| match err {
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
            }),
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
    file_service::read_document(&config, id).map_err(|e| match e {
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
        | FSReadDocumentError::FileEncryptionError(_)
        | FSReadDocumentError::FileDecompressionError(_) => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDocumentToDiskError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
    BadPath,
    FileAlreadyExistsInDisk,
}

pub fn save_document_to_disk(
    config: &Config,
    id: Uuid,
    location: String,
) -> Result<(), Error<SaveDocumentToDiskError>> {
    file_service::save_document_to_disk(&config, id, location).map_err(|e| match e {
        FSSaveDocumentToDiskError::CouldNotCreateDocumentError(creation_err) => {
            match creation_err.kind() {
                ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::InvalidInput => {
                    UiError(SaveDocumentToDiskError::BadPath)
                }
                ErrorKind::AlreadyExists => {
                    UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk)
                }
                _ => unexpected!("{:#?}", creation_err),
            }
        }
        FSSaveDocumentToDiskError::CouldNotWriteToDocumentError(_) => {
            unexpected!("{:#?}", e)
        }
        FSSaveDocumentToDiskError::ReadDocumentError(read_document_err) => {
            match read_document_err {
                FSReadDocumentError::TreatedFolderAsDocument => {
                    UiError(SaveDocumentToDiskError::TreatedFolderAsDocument)
                }

                FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount => UiError(SaveDocumentToDiskError::NoAccount),
                    AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                        unexpected!("{:#?}", account_error)
                    }
                },
                FSReadDocumentError::CouldNotFindFile => {
                    UiError(SaveDocumentToDiskError::FileDoesNotExist)
                }
                FSReadDocumentError::DbError(_)
                | FSReadDocumentError::DocumentReadError(_)
                | FSReadDocumentError::CouldNotFindParents(_)
                | FSReadDocumentError::FileEncryptionError(_)
                | FSReadDocumentError::FileDecompressionError(_) => {
                    unexpected!("{:#?}", read_document_err)
                }
            }
        }
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
    file_metadata_repo::get_all_paths(&config, filter).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListMetadatasError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_metadatas(config: &Config) -> Result<Vec<FileMetadata>, Error<ListMetadatasError>> {
    file_metadata_repo::get_all(&config).map_err(|e| unexpected!("{:#?}", e))
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
    file_service::rename_file(&config, id, new_name).map_err(|e| match e {
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
    file_service::move_file(&config, id, new_parent).map_err(|e| match e {
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
}

pub fn sync_all(
    config: &Config,
    f: Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), Error<SyncAllError>> {
    sync_service::sync(&config, f).map_err(|e| match e {
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
        SyncError::WorkExecutionError(_) | SyncError::MetadataUpdateError(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLocalChangesError {
    Stub,
}

pub fn get_local_changes(config: &Config) -> Result<Vec<Uuid>, Error<GetLocalChangesError>> {
    Ok(local_changes_repo::get_all_local_changes(&config)
        .map_err(|err| match err {
            local_changes_repo::DbError::TimeError(_)
            | local_changes_repo::DbError::BackendError(_)
            | local_changes_repo::DbError::SerdeError(_) => {
                unexpected!("{:#?}", err)
            }
        })?
        .iter()
        .map(|change| change.id)
        .collect())
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, Error<CalculateWorkError>> {
    sync_service::calculate_work(&config).map_err(|e| match e {
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
pub enum SetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn set_last_synced(config: &Config, last_sync: u64) -> Result<(), Error<SetLastSyncedError>> {
    file_metadata_repo::set_last_synced(&config, last_sync).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_last_synced(config: &Config) -> Result<i64, Error<GetLastSyncedError>> {
    file_metadata_repo::get_last_updated(&config)
        .map(|n| n as i64)
        .map_err(|err| match err {
            DbError::BackendError(_) | DbError::SerdeError(_) => unexpected!("{:#?}", err),
        })
}

pub fn get_last_synced_human_string(config: &Config) -> Result<String, Error<GetLastSyncedError>> {
    let last_synced = get_last_synced(config)?;

    Ok(if last_synced != 0 {
        Duration::milliseconds(clock_service::get_time().0 - last_synced)
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
    usage_service::server_usage(&config)
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
    usage_service::get_usage_human_string(&config, exact).map_err(|err| match err {
        USGetUsageError::AccountRetrievalError(db_err) => match db_err {
            AccountRepoError::NoAccount => UiError(GetUsageError::NoAccount),
            AccountRepoError::SerdeError(_) | AccountRepoError::BackendError(_) => {
                unexpected!("{:#?}", db_err)
            }
        },
        USGetUsageError::ApiError(api_err) => match api_err {
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

pub fn get_local_and_server_usage(
    config: &Config,
    exact: bool,
) -> Result<LocalAndServerUsages, Error<GetUsageError>> {
    usage_service::local_and_server_usages(&config, exact).map_err(|err| match err {
        USLocalAndServerUsageError::GetUsageError(gue) => match gue {
            USGetUsageError::AccountRetrievalError(_) => UiError(GetUsageError::NoAccount),
            USGetUsageError::ApiError(api_err) => match api_err {
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
        },
        USLocalAndServerUsageError::CalcUncompressedError(_)
        | USLocalAndServerUsageError::UncompressedNumberTooLarge(_) => unexpected!("{:#?}", err),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetDrawingError {
    NoAccount,
    FolderTreatedAsDrawing,
    InvalidDrawing,
    FileDoesNotExist,
}

pub fn get_drawing(config: &Config, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
    drawing_service::get_drawing(&config, id).map_err(|e| match e {
        FSGetDrawingError::InvalidDrawingError(err) => match err.classify() {
            Category::Io => unexpected!("{:#?}", err),
            Category::Syntax | Category::Data | Category::Eof => {
                UiError(GetDrawingError::InvalidDrawing)
            }
        },
        FSGetDrawingError::FailedToRetrieveJson(err) => match err {
            FSReadDocumentError::TreatedFolderAsDocument => {
                UiError(GetDrawingError::FolderTreatedAsDrawing)
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
            | FSReadDocumentError::FileEncryptionError(_)
            | FSReadDocumentError::FileDecompressionError(_) => unexpected!("{:#?}", err),
        },
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDrawingError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDrawing,
    InvalidDrawing,
}

pub fn save_drawing(
    config: &Config,
    id: Uuid,
    drawing_bytes: &[u8],
) -> Result<(), Error<SaveDrawingError>> {
    drawing_service::save_drawing(&config, id, drawing_bytes).map_err(|e| match e {
        FSSaveDrawingError::InvalidDrawingError(err) => match err.classify() {
            Category::Io => unexpected!("{:#?}", err),
            Category::Syntax | Category::Data | Category::Eof => {
                UiError(SaveDrawingError::InvalidDrawing)
            }
        },
        FSSaveDrawingError::FailedToSaveJson(err) => match err {
            DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                    unexpected!("{:#?}", account_err)
                }
                AccountRepoError::NoAccount => UiError(SaveDrawingError::NoAccount),
            },
            DocumentUpdateError::CouldNotFindFile => UiError(SaveDrawingError::FileDoesNotExist),
            DocumentUpdateError::FolderTreatedAsDocument => {
                UiError(SaveDrawingError::FolderTreatedAsDrawing)
            }
            DocumentUpdateError::CouldNotFindParents(_)
            | DocumentUpdateError::FileEncryptionError(_)
            | DocumentUpdateError::FileCompressionError(_)
            | DocumentUpdateError::FileDecompressionError(_)
            | DocumentUpdateError::DocumentWriteError(_)
            | DocumentUpdateError::DbError(_)
            | DocumentUpdateError::FetchOldVersionError(_)
            | DocumentUpdateError::DecryptOldVersionError(_)
            | DocumentUpdateError::AccessInfoCreationError(_)
            | DocumentUpdateError::FailedToRecordChange(_) => unexpected!("{:#?}", err),
        },
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportDrawingError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    NoAccount,
    InvalidDrawing,
}

pub fn export_drawing(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, Error<ExportDrawingError>> {
    drawing_service::export_drawing(&config, id, format, render_theme).map_err(|e| match e {
        FSExportDrawingError::GetDrawingError(get_drawing_err) => match get_drawing_err {
            FSGetDrawingError::InvalidDrawingError(err) => match err.classify() {
                Category::Io => unexpected!("{:#?}", err),
                Category::Syntax | Category::Data | Category::Eof => {
                    UiError(ExportDrawingError::InvalidDrawing)
                }
            },
            FSGetDrawingError::FailedToRetrieveJson(err) => match err {
                FSReadDocumentError::TreatedFolderAsDocument => {
                    UiError(ExportDrawingError::FolderTreatedAsDrawing)
                }
                FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount => UiError(ExportDrawingError::NoAccount),
                    AccountRepoError::BackendError(_) | AccountRepoError::SerdeError(_) => {
                        unexpected!("{:#?}", account_error)
                    }
                },
                FSReadDocumentError::CouldNotFindFile => {
                    UiError(ExportDrawingError::FileDoesNotExist)
                }
                FSReadDocumentError::DbError(_)
                | FSReadDocumentError::DocumentReadError(_)
                | FSReadDocumentError::CouldNotFindParents(_)
                | FSReadDocumentError::FileEncryptionError(_)
                | FSReadDocumentError::FileDecompressionError(_) => {
                    unexpected!("{:#?}", err)
                }
            },
        },
        FSExportDrawingError::UnableToGetColorFromAlias
        | FSExportDrawingError::UnableToGetStrokePoint
        | FSExportDrawingError::UnableToGetStrokeGirth
        | FSExportDrawingError::UnequalPointsAndGirthMetrics
        | FSExportDrawingError::InvalidAlphaValue
        | FSExportDrawingError::FailedToEncodeImage(_) => {
            unexpected!("{:#?}", e)
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportDrawingToDiskError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    NoAccount,
    InvalidDrawing,
    BadPath,
    FileAlreadyExistsInDisk,
}

pub fn export_drawing_to_disk(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    location: String,
) -> Result<(), Error<ExportDrawingToDiskError>> {
    drawing_service::export_drawing_to_disk(&config, id, format, render_theme, location).map_err(
        |e| match e {
            FSExportDrawingToDiskError::CouldNotCreateDocumentError(creation_err) => {
                match creation_err.kind() {
                    ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::InvalidInput => {
                        UiError(ExportDrawingToDiskError::BadPath)
                    }
                    ErrorKind::AlreadyExists => {
                        UiError(ExportDrawingToDiskError::FileAlreadyExistsInDisk)
                    }
                    _ => unexpected!("{:#?}", creation_err),
                }
            }
            FSExportDrawingToDiskError::CouldNotWriteToDocumentError(_) => unexpected!("{:#?}", e),
            FSExportDrawingToDiskError::ExportDrawingError(export_drawing_err) => {
                match export_drawing_err {
                    FSExportDrawingError::GetDrawingError(get_drawing_err) => match get_drawing_err
                    {
                        FSGetDrawingError::InvalidDrawingError(err) => match err.classify() {
                            Category::Io => unexpected!("{:#?}", err),
                            Category::Syntax | Category::Data | Category::Eof => {
                                UiError(ExportDrawingToDiskError::InvalidDrawing)
                            }
                        },
                        FSGetDrawingError::FailedToRetrieveJson(err) => match err {
                            FSReadDocumentError::TreatedFolderAsDocument => {
                                UiError(ExportDrawingToDiskError::FolderTreatedAsDrawing)
                            }
                            FSReadDocumentError::AccountRetrievalError(account_error) => {
                                match account_error {
                                    AccountRepoError::NoAccount => {
                                        UiError(ExportDrawingToDiskError::NoAccount)
                                    }
                                    AccountRepoError::BackendError(_)
                                    | AccountRepoError::SerdeError(_) => {
                                        unexpected!("{:#?}", account_error)
                                    }
                                }
                            }
                            FSReadDocumentError::CouldNotFindFile => {
                                UiError(ExportDrawingToDiskError::FileDoesNotExist)
                            }
                            FSReadDocumentError::DbError(_)
                            | FSReadDocumentError::DocumentReadError(_)
                            | FSReadDocumentError::CouldNotFindParents(_)
                            | FSReadDocumentError::FileEncryptionError(_)
                            | FSReadDocumentError::FileDecompressionError(_) => {
                                unexpected!("{:#?}", err)
                            }
                        },
                    },
                    FSExportDrawingError::UnableToGetColorFromAlias
                    | FSExportDrawingError::UnableToGetStrokePoint
                    | FSExportDrawingError::UnableToGetStrokeGirth
                    | FSExportDrawingError::UnequalPointsAndGirthMetrics
                    | FSExportDrawingError::InvalidAlphaValue
                    | FSExportDrawingError::FailedToEncodeImage(_) => {
                        unexpected!("{:#?}", export_drawing_err)
                    }
                }
            }
        },
    )
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
    SetLastSyncedError,
    GetLastSyncedError,
    GetUsageError,
    GetDrawingError,
    SaveDrawingError,
    ExportDrawingError,
    ExportDrawingToDiskError,
    SaveDocumentToDiskError,
);

pub mod c_interface;
pub mod client;
pub mod java_interface;
mod json_interface;
pub mod loggers;
pub mod model;
pub mod repo;
pub mod service;

pub static DEFAULT_API_LOCATION: &str = "http://api.lockbook.app:8000";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
static LOG_FILE: &str = "lockbook.log";
