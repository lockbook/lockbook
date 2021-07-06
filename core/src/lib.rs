#![recursion_limit = "256"]

#[macro_use]
extern crate log;
extern crate reqwest;

use crate::client::ApiError;
use crate::model::client_conversion::{
    generate_client_file_metadata, generate_client_work_calculated, ClientFileMetadata,
    ClientWorkCalculated,
};
use crate::model::state::Config;
use crate::repo::local_changes_repo;
use crate::repo::{account_repo, file_metadata_repo};
use crate::service::db_state_service::State;
use crate::service::drawing_service::SupportedImageFormats;
use crate::service::sync_service::SyncProgress;
use crate::service::usage_service::{UsageItemMetric, UsageMetrics};
use crate::service::{
    account_service, db_state_service, drawing_service, file_service, path_service, sync_service,
    usage_service,
};
use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use lockbook_crypto::clock_service;
use lockbook_models::account::Account;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_models::file_metadata::{FileMetadata, FileType};
use serde::Serialize;
use serde_json::{json, value::Value};
use std::collections::HashMap;
use std::env;
use std::io::ErrorKind;
use std::path::Path;
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;
use Error::UiError;

#[derive(Debug, Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error<U: Serialize> {
    UiError(U),
    Unexpected(String),
}

macro_rules! unexpected {
    ($base:literal $(, $args:tt )*) => {
        Error::Unexpected(format!($base $(, $args )*))
    };
}

#[derive(Debug, Clone)]
pub enum CoreError {
    AccountExists,
    AccountNonexistent,
    AccountStringCorrupted,
    ClientUpdateRequired,
    ClientWipeRequired,
    DiskPathInvalid,
    DiskPathTaken,
    DrawingInvalid,
    FileExists,
    FileNameContainsSlash,
    FileNameEmpty,
    FileNonexistent,
    FileNotDocument,
    FileNotFolder,
    FileParentNonexistent,
    FolderMovedIntoSelf,
    PathContainsEmptyFileName,
    PathNonexistent,
    PathStartsWithNonRoot,
    PathTaken,
    RootModificationInvalid,
    RootNonexistent,
    ServerUnreachable,
    UsernameInvalid,
    UsernamePublicKeyMismatch,
    UsernameTaken,
    Unexpected(String),
}

fn core_err_unexpected<T: std::fmt::Debug>(err: T) -> CoreError {
    CoreError::Unexpected(format!("{:#?}", err))
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::InvalidInput => {
                CoreError::DiskPathInvalid
            }
            ErrorKind::AlreadyExists => CoreError::DiskPathTaken,
            _ => core_err_unexpected(e),
        }
    }
}

impl<T: std::fmt::Debug> From<ApiError<T>> for CoreError {
    fn from(e: ApiError<T>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
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
    db_state_service::get_state(&config).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MigrationError {
    StateRequiresCleaning,
}

pub fn migrate_db(config: &Config) -> Result<(), Error<MigrationError>> {
    db_state_service::perform_migration(&config).map_err(|e| match e {
        CoreError::ClientWipeRequired => UiError(MigrationError::StateRequiresCleaning),
        _ => unexpected!("{:#?}", e),
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
        CoreError::AccountExists => UiError(CreateAccountError::AccountExistsAlready),
        CoreError::UsernameTaken => UiError(CreateAccountError::UsernameTaken),
        CoreError::UsernameInvalid => UiError(CreateAccountError::InvalidUsername),
        CoreError::ServerUnreachable => UiError(CreateAccountError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(CreateAccountError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
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
        CoreError::AccountStringCorrupted => UiError(ImportError::AccountStringCorrupted),
        CoreError::AccountExists => UiError(ImportError::AccountExistsAlready),
        CoreError::UsernamePublicKeyMismatch => UiError(ImportError::UsernamePKMismatch),
        CoreError::ServerUnreachable => UiError(ImportError::CouldNotReachServer),
        CoreError::AccountNonexistent => UiError(ImportError::AccountDoesNotExist),
        CoreError::ClientUpdateRequired => UiError(ImportError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AccountExportError {
    NoAccount,
}

pub fn export_account(config: &Config) -> Result<String, Error<AccountExportError>> {
    account_service::export_account(&config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(AccountExportError::NoAccount),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

pub fn get_account(config: &Config) -> Result<Account, Error<GetAccountError>> {
    account_repo::get_account(&config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(GetAccountError::NoAccount),
        _ => unexpected!("{:#?}", e),
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
) -> Result<ClientFileMetadata, Error<CreateFileAtPathError>> {
    path_service::create_at_path(&config, path_and_name)
        .map_err(|e| match e {
            CoreError::PathStartsWithNonRoot => {
                UiError(CreateFileAtPathError::PathDoesntStartWithRoot)
            }
            CoreError::PathContainsEmptyFileName => {
                UiError(CreateFileAtPathError::PathContainsEmptyFile)
            }
            CoreError::RootNonexistent => UiError(CreateFileAtPathError::NoRoot),
            CoreError::AccountNonexistent => UiError(CreateFileAtPathError::NoAccount),
            CoreError::PathTaken => UiError(CreateFileAtPathError::FileAlreadyExists),
            CoreError::FileNotFolder => UiError(CreateFileAtPathError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", e),
        })
        .and_then(|file_metadata| {
            generate_client_file_metadata(config, &file_metadata)
                .map_err(|e| unexpected!("{:#?}", e))
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
        CoreError::AccountNonexistent => UiError(WriteToDocumentError::NoAccount),
        CoreError::FileNonexistent => UiError(WriteToDocumentError::FileDoesNotExist),
        CoreError::FileNotDocument => UiError(WriteToDocumentError::FolderTreatedAsDocument),
        _ => unexpected!("{:#?}", e),
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
) -> Result<ClientFileMetadata, Error<CreateFileError>> {
    file_service::create(&config, name, parent, file_type)
        .map_err(|e| match e {
            CoreError::AccountNonexistent => UiError(CreateFileError::NoAccount),
            CoreError::FileNotFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
            CoreError::FileParentNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            CoreError::PathTaken => UiError(CreateFileError::FileNameNotAvailable),
            CoreError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
            CoreError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
            _ => unexpected!("{:#?}", e),
        })
        .and_then(|file_metadata| {
            generate_client_file_metadata(config, &file_metadata)
                .map_err(|e| unexpected!("{:#?}", e))
        })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

pub fn get_root(config: &Config) -> Result<ClientFileMetadata, Error<GetRootError>> {
    match file_metadata_repo::get_root(&config) {
        Ok(file_metadata) => match file_metadata {
            None => Err(UiError(GetRootError::NoRoot)),
            Some(file_metadata) => match generate_client_file_metadata(config, &file_metadata) {
                Ok(client_file_metadata) => Ok(client_file_metadata),
                Err(err) => Err(unexpected!("{:#?}", err)),
            },
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
) -> Result<Vec<ClientFileMetadata>, Error<GetChildrenError>> {
    let children: Vec<FileMetadata> = file_metadata_repo::get_children_non_recursively(&config, id)
        .map_err(|e| unexpected!("{:#?}", e))?;

    let mut client_children = vec![];

    for child in children {
        client_children.push(
            generate_client_file_metadata(config, &child).map_err(|e| unexpected!("{:#?}", e))?,
        );
    }

    Ok(client_children)
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
        CoreError::FileNonexistent => UiError(GetAndGetChildrenError::FileDoesNotExist),
        CoreError::FileNotFolder => UiError(GetAndGetChildrenError::DocumentTreatedAsFolder),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByIdError {
    NoFileWithThatId,
}

pub fn get_file_by_id(
    config: &Config,
    id: Uuid,
) -> Result<ClientFileMetadata, Error<GetFileByIdError>> {
    file_metadata_repo::get(&config, id)
        .map_err(|e| match e {
            CoreError::FileNonexistent => UiError(GetFileByIdError::NoFileWithThatId),
            _ => unexpected!("{:#?}", e),
        })
        .and_then(|file_metadata| {
            generate_client_file_metadata(config, &file_metadata)
                .map_err(|e| unexpected!("{:#?}", e))
        })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

pub fn get_file_by_path(
    config: &Config,
    path: &str,
) -> Result<ClientFileMetadata, Error<GetFileByPathError>> {
    path_service::get_by_path(&config, path)
        .map_err(|e| match e {
            CoreError::FileNonexistent => UiError(GetFileByPathError::NoFileAtThatPath),
            _ => unexpected!("{:#?}", e),
        })
        .and_then(|file_metadata| {
            generate_client_file_metadata(config, &file_metadata)
                .map_err(|e| unexpected!("{:#?}", e))
        })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), Error<FileDeleteError>> {
    match file_metadata_repo::get(&config, id) {
        Ok(meta) => match meta.file_type {
            FileType::Document => file_service::delete_document(&config, id),
            FileType::Folder => file_service::delete_folder(&config, id),
        }
        .map_err(|e| match e {
            CoreError::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
            CoreError::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }),
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
        CoreError::FileNotDocument => UiError(ReadDocumentError::TreatedFolderAsDocument),
        CoreError::AccountNonexistent => UiError(ReadDocumentError::NoAccount),
        CoreError::FileNonexistent => UiError(ReadDocumentError::FileDoesNotExist),
        _ => unexpected!("{:#?}", e),
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
        CoreError::FileNotDocument => UiError(SaveDocumentToDiskError::TreatedFolderAsDocument),
        CoreError::AccountNonexistent => UiError(SaveDocumentToDiskError::NoAccount),
        CoreError::FileNonexistent => UiError(SaveDocumentToDiskError::FileDoesNotExist),
        CoreError::DiskPathInvalid => UiError(SaveDocumentToDiskError::BadPath),
        CoreError::DiskPathTaken => UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListPathsError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_paths(
    config: &Config,
    filter: Option<path_service::Filter>,
) -> Result<Vec<String>, Error<ListPathsError>> {
    path_service::get_all_paths(&config, filter).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetPathError {
    Stub, // TODO: Enums should not be empty
}

pub fn get_path_by_id(config: &Config, uuid: Uuid) -> Result<String, Error<GetPathError>> {
    path_service::get_path_by_id(config, uuid).map_err(|e| unexpected!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ListMetadatasError {
    Stub, // TODO: Enums should not be empty
}

pub fn list_metadatas(
    config: &Config,
) -> Result<Vec<ClientFileMetadata>, Error<ListMetadatasError>> {
    let metas = file_metadata_repo::get_all(&config).map_err(|e| unexpected!("{:#?}", e))?;
    let mut client_metas = vec![];

    for meta in metas {
        client_metas.push(
            generate_client_file_metadata(config, &meta).map_err(|e| unexpected!("{:#?}", e))?,
        );
    }

    Ok(client_metas)
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
        CoreError::FileNonexistent => UiError(RenameFileError::FileDoesNotExist),
        CoreError::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
        CoreError::FileNameContainsSlash => UiError(RenameFileError::NewNameContainsSlash),
        CoreError::PathTaken => UiError(RenameFileError::FileNameNotAvailable),
        CoreError::RootModificationInvalid => UiError(RenameFileError::CannotRenameRoot),
        _ => unexpected!("{:#?}", e),
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
        CoreError::RootModificationInvalid => UiError(MoveFileError::CannotMoveRoot),
        CoreError::FileNotFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
        CoreError::FileNonexistent => UiError(MoveFileError::FileDoesNotExist),
        CoreError::FolderMovedIntoSelf => UiError(MoveFileError::FolderMovedIntoItself),
        CoreError::AccountNonexistent => UiError(MoveFileError::NoAccount),
        CoreError::FileParentNonexistent => UiError(MoveFileError::TargetParentDoesNotExist),
        CoreError::PathTaken => UiError(MoveFileError::TargetParentHasChildNamedThat),
        _ => unexpected!("{:#?}", e),
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
        CoreError::AccountNonexistent => UiError(SyncAllError::NoAccount),
        CoreError::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLocalChangesError {
    Stub,
}

pub fn get_local_changes(config: &Config) -> Result<Vec<Uuid>, Error<GetLocalChangesError>> {
    Ok(local_changes_repo::get_all_local_changes(&config)
        .map_err(|err| unexpected!("{:#?}", err))?
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

pub fn calculate_work(config: &Config) -> Result<ClientWorkCalculated, Error<CalculateWorkError>> {
    sync_service::calculate_work(&config)
        .map_err(|e| match e {
            CoreError::AccountNonexistent => UiError(CalculateWorkError::NoAccount),
            CoreError::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        })
        .and_then(|work_calculated| {
            generate_client_work_calculated(config, &work_calculated)
                .map_err(|e| unexpected!("{:#?}", e))
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
        .map_err(|e| unexpected!("{:#?}", e))
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

pub fn get_usage(config: &Config) -> Result<UsageMetrics, Error<GetUsageError>> {
    usage_service::get_usage(&config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(GetUsageError::NoAccount),
        CoreError::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

pub fn get_uncompressed_usage(config: &Config) -> Result<UsageItemMetric, Error<GetUsageError>> {
    usage_service::get_uncompressed_usage(&config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(GetUsageError::NoAccount),
        CoreError::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
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
        CoreError::DrawingInvalid => UiError(GetDrawingError::InvalidDrawing),
        CoreError::FileNotDocument => UiError(GetDrawingError::FolderTreatedAsDrawing),
        CoreError::AccountNonexistent => UiError(GetDrawingError::NoAccount),
        CoreError::FileNonexistent => UiError(GetDrawingError::FileDoesNotExist),
        _ => unexpected!("{:#?}", e),
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
        CoreError::DrawingInvalid => UiError(SaveDrawingError::InvalidDrawing),
        CoreError::AccountNonexistent => UiError(SaveDrawingError::NoAccount),
        CoreError::FileNonexistent => UiError(SaveDrawingError::FileDoesNotExist),
        CoreError::FileNotDocument => UiError(SaveDrawingError::FolderTreatedAsDrawing),
        _ => unexpected!("{:#?}", e),
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
        CoreError::DrawingInvalid => UiError(ExportDrawingError::InvalidDrawing),
        CoreError::AccountNonexistent => UiError(ExportDrawingError::NoAccount),
        CoreError::FileNonexistent => UiError(ExportDrawingError::FileDoesNotExist),
        CoreError::FileNotDocument => UiError(ExportDrawingError::FolderTreatedAsDrawing),
        _ => unexpected!("{:#?}", e),
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
            CoreError::DrawingInvalid => UiError(ExportDrawingToDiskError::InvalidDrawing),
            CoreError::AccountNonexistent => UiError(ExportDrawingToDiskError::NoAccount),
            CoreError::FileNonexistent => UiError(ExportDrawingToDiskError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(ExportDrawingToDiskError::FolderTreatedAsDrawing),
            CoreError::DiskPathInvalid => UiError(ExportDrawingToDiskError::BadPath),
            CoreError::DiskPathTaken => UiError(ExportDrawingToDiskError::FileAlreadyExistsInDisk),
            _ => unexpected!("{:#?}", e),
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

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
static LOG_FILE: &str = "lockbook.log";
