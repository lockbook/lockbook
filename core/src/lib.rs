#![recursion_limit = "256"]

#[macro_use]
extern crate log;
extern crate reqwest;

use std::env;
use std::path::Path;
use std::str::FromStr;

use serde::Serialize;
use serde_json::json;
use serde_json::value::Value;
pub use sled::Db;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;

use crate::client::{ApiError, Client, ClientImpl};
use crate::model::account::Account;
use crate::model::api::{FileUsage, GetPublicKeyError, GetUsageRequest, NewAccountError};
use crate::model::crypto::DecryptedDocument;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::repo::account_repo::{AccountRepo, AccountRepoError, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::db_version_repo::DbVersionRepoImpl;
use crate::repo::document_repo::DocumentRepoImpl;
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::{
    DbError, FileMetadataRepo, FileMetadataRepoImpl, Filter, FindingChildrenFailed,
    FindingParentsFailed,
};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::AccountExportError as ASAccountExportError;
use crate::service::account_service::{
    AccountCreationError, AccountImportError, AccountService, AccountServiceImpl,
};
use crate::service::clock_service::ClockImpl;
use crate::service::code_version_service::CodeVersionImpl;
use crate::service::crypto_service::{AESImpl, RSAImpl};
use crate::service::db_state_service;
use crate::service::db_state_service::{DbStateService, DbStateServiceImpl, State};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service;
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

#[derive(Debug, Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error<U: Serialize> {
    UiError(U),
    Unexpected(String),
}

pub fn init_logger(log_path: &Path) -> Result<(), Error<()>> {
    let print_colors = env::var("LOG_NO_COLOR").is_err();
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| log::LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or_else(|| log::LevelFilter::Debug);

    loggers::init(log_path, LOG_FILE.to_string(), print_colors)
        .map_err(|err| Error::Unexpected(format!("IO Error: {:#?}", err)))?
        .level(log::LevelFilter::Warn)
        .level_for("lockbook_core", lockbook_log_level)
        .apply()
        .map_err(|err| Error::Unexpected(format!("{:#?}", err)))?;
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

#[derive(Debug, Serialize, EnumIter)]
pub enum GetStateError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_db_state(config: &Config) -> Result<State, Error<GetStateError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultDbStateService::get_state(&db) {
        Ok(state) => Ok(state),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MigrationError {
    StateRequiresCleaning,
}

pub fn migrate_db(config: &Config) -> Result<(), Error<MigrationError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultDbStateService::perform_migration(&db) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            db_state_service::MigrationError::StateRequiresClearing => {
                Err(Error::UiError(MigrationError::StateRequiresCleaning))
            }
            db_state_service::MigrationError::RepoError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
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
) -> Result<(), Error<CreateAccountError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultAccountService::create_account(&db, username, api_url) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountCreationError::AccountExistsAlready => {
                Err(Error::UiError(CreateAccountError::AccountExistsAlready))
            }
            AccountCreationError::ApiError(network) => match network {
                ApiError::Endpoint(api_err) => match api_err {
                    NewAccountError::UsernameTaken => {
                        Err(Error::UiError(CreateAccountError::UsernameTaken))
                    }
                    NewAccountError::InvalidUsername => {
                        Err(Error::UiError(CreateAccountError::InvalidUsername))
                    }
                    NewAccountError::InvalidPublicKey
                    | NewAccountError::InvalidUserAccessKey
                    | NewAccountError::FileIdTaken => {
                        Err(Error::Unexpected(format!("{:#?}", api_err)))
                    }
                },
                ApiError::SendFailed(_) => {
                    Err(Error::UiError(CreateAccountError::CouldNotReachServer))
                }
                ApiError::ClientUpdateRequired => {
                    Err(Error::UiError(CreateAccountError::ClientUpdateRequired))
                }
                ApiError::Serialize(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_)
                | ApiError::Sign(_)
                | ApiError::InternalError
                | ApiError::BadRequest
                | ApiError::InvalidAuth
                | ApiError::ExpiredAuth => Err(Error::Unexpected(format!("{:#?}", network))),
            },
            AccountCreationError::KeyGenerationError(_)
            | AccountCreationError::AccountRepoError(_)
            | AccountCreationError::FolderError(_)
            | AccountCreationError::MetadataRepoError(_)
            | AccountCreationError::KeySerializationError(_)
            | AccountCreationError::AccountRepoDbError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
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

pub fn import_account(config: &Config, account_string: &str) -> Result<(), Error<ImportError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultAccountService::import_account(&db, account_string) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            AccountImportError::AccountStringCorrupted(_)
            | AccountImportError::AccountStringFailedToDeserialize(_)
            | AccountImportError::InvalidPrivateKey(_) => {
                Err(Error::UiError(ImportError::AccountStringCorrupted))
            }
            AccountImportError::AccountExistsAlready => {
                Err(Error::UiError(ImportError::AccountExistsAlready))
            }
            AccountImportError::PublicKeyMismatch => {
                Err(Error::UiError(ImportError::UsernamePKMismatch))
            }
            AccountImportError::FailedToVerifyAccountServerSide(client_err) => match client_err {
                ApiError::SendFailed(_) => Err(Error::UiError(ImportError::CouldNotReachServer)),
                ApiError::Endpoint(api_err) => match api_err {
                    GetPublicKeyError::UserNotFound => {
                        Err(Error::UiError(ImportError::AccountDoesNotExist))
                    }
                    GetPublicKeyError::InvalidUsername => {
                        Err(Error::Unexpected(format!("{:#?}", api_err)))
                    }
                },
                ApiError::ClientUpdateRequired => {
                    Err(Error::UiError(ImportError::ClientUpdateRequired))
                }
                ApiError::Serialize(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_)
                | ApiError::Sign(_)
                | ApiError::InternalError
                | ApiError::BadRequest
                | ApiError::InvalidAuth
                | ApiError::ExpiredAuth => Err(Error::Unexpected(format!("{:#?}", client_err))),
            },
            AccountImportError::PersistenceError(_) | AccountImportError::AccountRepoDbError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AccountExportError {
    NoAccount,
}

pub fn export_account(config: &Config) -> Result<String, Error<AccountExportError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultAccountService::export_account(&db) {
        Ok(account_string) => Ok(account_string),
        Err(err) => match err {
            ASAccountExportError::AccountRetrievalError(db_err) => match db_err {
                AccountRepoError::NoAccount => Err(Error::UiError(AccountExportError::NoAccount)),
                AccountRepoError::SerdeError(_) | AccountRepoError::SledError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", db_err)))
                }
            },
            ASAccountExportError::AccountStringFailedToSerialize(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

pub fn get_account(config: &Config) -> Result<Account, Error<GetAccountError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => Ok(account),
        Err(err) => match err {
            AccountRepoError::NoAccount => Err(Error::UiError(GetAccountError::NoAccount)),
            AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::create_at_path(&db, path_and_name) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            NewFileFromPathError::PathDoesntStartWithRoot => Err(Error::UiError(
                CreateFileAtPathError::PathDoesntStartWithRoot,
            )),
            NewFileFromPathError::PathContainsEmptyFile => {
                Err(Error::UiError(CreateFileAtPathError::PathContainsEmptyFile))
            }
            NewFileFromPathError::FileAlreadyExists => {
                Err(Error::UiError(CreateFileAtPathError::FileAlreadyExists))
            }
            NewFileFromPathError::NoRoot => Err(Error::UiError(CreateFileAtPathError::NoRoot)),
            NewFileFromPathError::FailedToCreateChild(failed_to_create) => match failed_to_create {
                NewFileError::AccountRetrievalError(account_error) => match account_error {
                    AccountRepoError::NoAccount => {
                        Err(Error::UiError(CreateFileAtPathError::NoAccount))
                    }
                    AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                        Err(Error::Unexpected(format!("{:#?}", account_error)))
                    }
                },
                NewFileError::FileNameNotAvailable => {
                    Err(Error::UiError(CreateFileAtPathError::FileAlreadyExists))
                }
                NewFileError::DocumentTreatedAsFolder => Err(Error::UiError(
                    CreateFileAtPathError::DocumentTreatedAsFolder,
                )),
                NewFileError::CouldNotFindParents(_)
                | NewFileError::FileCryptoError(_)
                | NewFileError::MetadataRepoError(_)
                | NewFileError::FailedToWriteFileContent(_)
                | NewFileError::FailedToRecordChange(_)
                | NewFileError::FileNameEmpty
                | NewFileError::FileNameContainsSlash => {
                    Err(Error::Unexpected(format!("{:#?}", failed_to_create)))
                }
            },
            NewFileFromPathError::FailedToRecordChange(_) | NewFileFromPathError::DbError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::write_document(&db, id, content) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            DocumentUpdateError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", account_err)))
                }
                AccountRepoError::NoAccount => Err(Error::UiError(WriteToDocumentError::NoAccount)),
            },
            DocumentUpdateError::CouldNotFindFile => {
                Err(Error::UiError(WriteToDocumentError::FileDoesNotExist))
            }
            DocumentUpdateError::FolderTreatedAsDocument => Err(Error::UiError(
                WriteToDocumentError::FolderTreatedAsDocument,
            )),
            DocumentUpdateError::CouldNotFindParents(_)
            | DocumentUpdateError::FileCryptoError(_)
            | DocumentUpdateError::DocumentWriteError(_)
            | DocumentUpdateError::DbError(_)
            | DocumentUpdateError::FetchOldVersionError(_)
            | DocumentUpdateError::DecryptOldVersionError(_)
            | DocumentUpdateError::AccessInfoCreationError(_)
            | DocumentUpdateError::FailedToRecordChange(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::create(&db, name, parent, file_type) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            NewFileError::AccountRetrievalError(_) => {
                Err(Error::UiError(CreateFileError::NoAccount))
            }
            NewFileError::DocumentTreatedAsFolder => {
                Err(Error::UiError(CreateFileError::DocumentTreatedAsFolder))
            }
            NewFileError::CouldNotFindParents(parent_error) => match parent_error {
                FindingParentsFailed::AncestorMissing => {
                    Err(Error::UiError(CreateFileError::CouldNotFindAParent))
                }
                FindingParentsFailed::DbError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", parent_error)))
                }
            },
            NewFileError::FileNameNotAvailable => {
                Err(Error::UiError(CreateFileError::FileNameNotAvailable))
            }
            NewFileError::FileNameEmpty => Err(Error::UiError(CreateFileError::FileNameEmpty)),
            NewFileError::FileNameContainsSlash => {
                Err(Error::UiError(CreateFileError::FileNameContainsSlash))
            }
            NewFileError::FileCryptoError(_)
            | NewFileError::MetadataRepoError(_)
            | NewFileError::FailedToWriteFileContent(_)
            | NewFileError::FailedToRecordChange(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

pub fn get_root(config: &Config) -> Result<FileMetadata, Error<GetRootError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_root(&db) {
        Ok(file_metadata) => match file_metadata {
            None => Err(Error::UiError(GetRootError::NoRoot)),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_children_non_recursively(&db, id) {
        Ok(file_metadata_list) => Ok(file_metadata_list),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_and_get_children_recursively(&db, id) {
        Ok(children) => Ok(children),
        Err(err) => match err {
            FindingChildrenFailed::FileDoesNotExist => {
                Err(Error::UiError(GetAndGetChildrenError::FileDoesNotExist))
            }
            FindingChildrenFailed::DocumentTreatedAsFolder => Err(Error::UiError(
                GetAndGetChildrenError::DocumentTreatedAsFolder,
            )),
            FindingChildrenFailed::DbError(err) => Err(Error::Unexpected(format!("{:#?}", err))),
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByIdError {
    NoFileWithThatId,
}

pub fn get_file_by_id(config: &Config, id: Uuid) -> Result<FileMetadata, Error<GetFileByIdError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get(&db, id) {
        Ok(file_metadata) => Ok(file_metadata),
        Err(err) => match err {
            file_metadata_repo::Error::FileRowMissing(_) => {
                Err(Error::UiError(GetFileByIdError::NoFileWithThatId))
            }
            file_metadata_repo::Error::SledError(_) | file_metadata_repo::Error::SerdeError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

pub fn get_file_by_path(
    config: &Config,
    path: &str,
) -> Result<FileMetadata, Error<GetFileByPathError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_by_path(&db, path) {
        Ok(maybe_file_metadata) => match maybe_file_metadata {
            None => Err(Error::UiError(GetFileByPathError::NoFileAtThatPath)),
            Some(file_metadata) => Ok(file_metadata),
        },
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match FileMetadataRepoImpl::insert(&db, &file_metadata) {
        Ok(()) => Ok(()),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    FileDoesNotExist,
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), Error<FileDeleteError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get(&db, id) {
        Ok(meta) => match meta.file_type {
            FileType::Document => {
                DefaultFileService::delete_document(&db, id).map_err(|err| match err {
                    file_service::DeleteDocumentError::CouldNotFindFile
                    | file_service::DeleteDocumentError::FolderTreatedAsDocument
                    | file_service::DeleteDocumentError::FailedToRecordChange(_)
                    | file_service::DeleteDocumentError::FailedToUpdateMetadata(_)
                    | file_service::DeleteDocumentError::FailedToDeleteDocument(_)
                    | file_service::DeleteDocumentError::FailedToTrackDelete(_)
                    | file_service::DeleteDocumentError::DbError(_) => {
                        Error::Unexpected(format!("{:#?}", err))
                    }
                })
            }
            FileType::Folder => {
                DefaultFileService::delete_folder(&db, id).map_err(|err| match err {
                    file_service::DeleteFolderError::MetadataError(_)
                    | file_service::DeleteFolderError::CouldNotFindFile
                    | file_service::DeleteFolderError::FailedToDeleteMetadata(_)
                    | file_service::DeleteFolderError::FindingChildrenFailed(_)
                    | file_service::DeleteFolderError::FailedToRecordChange(_)
                    | file_service::DeleteFolderError::CouldNotFindParents(_)
                    | file_service::DeleteFolderError::DocumentTreatedAsFolder
                    | file_service::DeleteFolderError::FailedToDeleteDocument(_)
                    | file_service::DeleteFolderError::FailedToDeleteChangeEntry(_) => {
                        Error::Unexpected(format!("{:#?}", err))
                    }
                })
            }
        },
        Err(_) => Err(Error::UiError(FileDeleteError::FileDoesNotExist)),
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::read_document(&db, id) {
        Ok(decrypted) => Ok(decrypted),
        Err(err) => match err {
            FSReadDocumentError::TreatedFolderAsDocument => {
                Err(Error::UiError(ReadDocumentError::TreatedFolderAsDocument))
            }

            FSReadDocumentError::AccountRetrievalError(account_error) => match account_error {
                AccountRepoError::NoAccount => Err(Error::UiError(ReadDocumentError::NoAccount)),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", account_error)))
                }
            },
            FSReadDocumentError::CouldNotFindFile => {
                Err(Error::UiError(ReadDocumentError::FileDoesNotExist))
            }
            FSReadDocumentError::DbError(_)
            | FSReadDocumentError::DocumentReadError(_)
            | FSReadDocumentError::CouldNotFindParents(_)
            | FSReadDocumentError::FileCryptoError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListPathsError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_paths(
    config: &Config,
    filter: Option<Filter>,
) -> Result<Vec<String>, Error<ListPathsError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_all_paths(&db, filter) {
        Ok(paths) => Ok(paths),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListMetadatasError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_metadatas(config: &Config) -> Result<Vec<FileMetadata>, Error<ListMetadatasError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_all(&db) {
        Ok(metas) => Ok(metas),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::rename_file(&db, id, new_name) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            DocumentRenameError::FileDoesNotExist => {
                Err(Error::UiError(RenameFileError::FileDoesNotExist))
            }
            DocumentRenameError::FileNameEmpty => {
                Err(Error::UiError(RenameFileError::NewNameEmpty))
            }
            DocumentRenameError::FileNameContainsSlash => {
                Err(Error::UiError(RenameFileError::NewNameContainsSlash))
            }
            DocumentRenameError::FileNameNotAvailable => {
                Err(Error::UiError(RenameFileError::FileNameNotAvailable))
            }
            DocumentRenameError::CannotRenameRoot => {
                Err(Error::UiError(RenameFileError::CannotRenameRoot))
            }
            DocumentRenameError::DbError(_) | DocumentRenameError::FailedToRecordChange(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MoveFileError {
    NoAccount,
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
    CannotMoveRoot,
}

pub fn move_file(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileService::move_file(&db, id, new_parent) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            FileMoveError::DocumentTreatedAsFolder => {
                Err(Error::UiError(MoveFileError::DocumentTreatedAsFolder))
            }
            FileMoveError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::NoAccount => Err(Error::UiError(MoveFileError::NoAccount)),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", account_err)))
                }
            },
            FileMoveError::TargetParentHasChildNamedThat => {
                Err(Error::UiError(MoveFileError::TargetParentHasChildNamedThat))
            }
            FileMoveError::FileDoesNotExist => Err(Error::UiError(MoveFileError::FileDoesNotExist)),
            FileMoveError::TargetParentDoesNotExist => {
                Err(Error::UiError(MoveFileError::TargetParentDoesNotExist))
            }
            FileMoveError::CannotMoveRoot => Err(Error::UiError(MoveFileError::CannotMoveRoot)),
            FileMoveError::DbError(_)
            | FileMoveError::FailedToRecordChange(_)
            | FileMoveError::FailedToDecryptKey(_)
            | FileMoveError::FailedToReEncryptKey(_)
            | FileMoveError::CouldNotFindParents(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SyncAllError {
    NoAccount,
    ClientUpdateRequired,
    CouldNotReachServer,
    ExecuteWorkError, // TODO: @parth ExecuteWorkError(Vec<Error<ExecuteWorkError>>),
}

pub fn sync_all(config: &Config) -> Result<(), Error<SyncAllError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultSyncService::sync(&db) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            SyncError::AccountRetrievalError(err) => match err {
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", err)))
                }
                AccountRepoError::NoAccount => Err(Error::UiError(SyncAllError::NoAccount)),
            },
            SyncError::CalculateWorkError(err) => match err {
                SSCalculateWorkError::LocalChangesRepoError(_)
                | SSCalculateWorkError::MetadataRepoError(_)
                | SSCalculateWorkError::GetMetadataError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", err)))
                }
                SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
                    AccountRepoError::NoAccount => Err(Error::UiError(SyncAllError::NoAccount)),
                    AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                        Err(Error::Unexpected(format!("{:#?}", account_err)))
                    }
                },
                SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
                    ApiError::SendFailed(_) => {
                        Err(Error::UiError(SyncAllError::CouldNotReachServer))
                    }
                    ApiError::ClientUpdateRequired => {
                        Err(Error::UiError(SyncAllError::ClientUpdateRequired))
                    }
                    ApiError::Serialize(_)
                    | ApiError::ReceiveFailed(_)
                    | ApiError::Deserialize(_)
                    | ApiError::Sign(_)
                    | ApiError::InternalError
                    | ApiError::BadRequest
                    | ApiError::InvalidAuth
                    | ApiError::ExpiredAuth
                    | ApiError::Endpoint(_) => Err(Error::Unexpected(format!("{:#?}", api_err))),
                },
            },
            SyncError::WorkExecutionError(_) => Err(Error::UiError(SyncAllError::ExecuteWorkError)),
            SyncError::MetadataUpdateError(err) => Err(Error::Unexpected(format!("{:#?}", err))),
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, Error<CalculateWorkError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultSyncService::calculate_work(&db) {
        Ok(work) => Ok(work),
        Err(err) => match err {
            SSCalculateWorkError::LocalChangesRepoError(_)
            | SSCalculateWorkError::MetadataRepoError(_)
            | SSCalculateWorkError::GetMetadataError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
            SSCalculateWorkError::AccountRetrievalError(account_err) => match account_err {
                AccountRepoError::NoAccount => Err(Error::UiError(CalculateWorkError::NoAccount)),
                AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                    Err(Error::Unexpected(format!("{:#?}", account_err)))
                }
            },
            SSCalculateWorkError::GetUpdatesError(api_err) => match api_err {
                ApiError::SendFailed(_) => {
                    Err(Error::UiError(CalculateWorkError::CouldNotReachServer))
                }
                ApiError::ClientUpdateRequired => {
                    Err(Error::UiError(CalculateWorkError::ClientUpdateRequired))
                }
                ApiError::Serialize(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_)
                | ApiError::Sign(_)
                | ApiError::InternalError
                | ApiError::BadRequest
                | ApiError::InvalidAuth
                | ApiError::ExpiredAuth
                | ApiError::Endpoint(_) => Err(Error::Unexpected(format!("{:#?}", api_err))),
            },
        },
    }
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
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultSyncService::execute_work(&db, &account, wu) {
        Ok(_) => Ok(()),
        Err(err) => match err {
            WorkExecutionError::SendFailed(_) => {
                Err(Error::UiError(ExecuteWorkError::CouldNotReachServer))
            }
            WorkExecutionError::ClientUpdateRequired => {
                Err(Error::UiError(ExecuteWorkError::ClientUpdateRequired))
            }
            WorkExecutionError::MetadataRepoError(_)
            | WorkExecutionError::MetadataRepoErrorOpt(_)
            | WorkExecutionError::SaveDocumentError(_)
            | WorkExecutionError::AutoRenameError(_)
            | WorkExecutionError::ResolveConflictByCreatingNewFileError(_)
            | WorkExecutionError::DecryptingOldVersionForMergeError(_)
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
            | WorkExecutionError::FolderDeleteError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn set_last_synced(config: &Config, last_sync: u64) -> Result<(), Error<SetLastSyncedError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::set_last_synced(&db, last_sync) {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::Unexpected(format!("{:#?}", err))),
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLastSyncedError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_last_synced(config: &Config) -> Result<i64, Error<GetLastSyncedError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    match DefaultFileMetadataRepo::get_last_updated(&db) {
        Ok(val) => Ok(val as i64),
        Err(err) => match err {
            DbError::SledError(_) | DbError::SerdeError(_) => {
                Err(Error::Unexpected(format!("{:#?}", err)))
            }
        },
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetUsageError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn get_usage(config: &Config) -> Result<Vec<FileUsage>, Error<GetUsageError>> {
    let db = connect_to_db(&config).map_err(Error::Unexpected)?;

    let acc = DefaultAccountRepo::get_account(&db)
        .map_err(|_| Error::UiError(GetUsageError::NoAccount))?;

    DefaultClient::request(&acc, GetUsageRequest {})
        .map(|resp| resp.usages)
        .map_err(|err| match err {
            ApiError::SendFailed(_) => Error::UiError(GetUsageError::CouldNotReachServer),
            ApiError::ClientUpdateRequired => Error::UiError(GetUsageError::ClientUpdateRequired),
            ApiError::Endpoint(_)
            | ApiError::InvalidAuth
            | ApiError::ExpiredAuth
            | ApiError::InternalError
            | ApiError::BadRequest
            | ApiError::Sign(_)
            | ApiError::Serialize(_)
            | ApiError::ReceiveFailed(_)
            | ApiError::Deserialize(_) => Error::Unexpected(format!("{:#?}", err)),
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

pub static DEFAULT_API_LOCATION: &str = "http://api.lockbook.app:8000";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
static DB_NAME: &str = "lockbook.sled";
static LOG_FILE: &str = "lockbook.log";

pub type DefaultClock = ClockImpl;
pub type DefaultCrypto = RSAImpl<DefaultClock>;
pub type DefaultSymmetric = AESImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultCodeVersion = CodeVersionImpl;
pub type DefaultClient = ClientImpl<DefaultCrypto, DefaultCodeVersion>;
pub type DefaultAccountRepo = AccountRepoImpl;
pub type DefaultDbVersionRepo = DbVersionRepoImpl;
pub type DefaultDbStateService =
    DbStateServiceImpl<DefaultAccountRepo, DefaultDbVersionRepo, DefaultCodeVersion>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultLocalChangesRepo = LocalChangesRepoImpl<DefaultClock>;
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
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultLocalChangesRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;
