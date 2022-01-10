#![recursion_limit = "256"]

#[macro_use]
extern crate log;
extern crate reqwest;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;

use serde::Serialize;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;

use lockbook_crypto::clock_service;
use lockbook_models::account::Account;
use lockbook_models::api::{AccountTier, CreditCardInfo, InvalidCreditCardType};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_models::file_metadata::{DecryptedFileMetadata, EncryptedFileMetadata, FileType};
use model::errors::Error::UiError;
pub use model::errors::{CoreError, Error, UnexpectedError};
use service::log_service;

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::drawing::SupportedImageFormats;
use crate::repo::{account_repo, last_updated_repo};
use crate::service::db_state_service::State;
use crate::service::import_export_service::{self, ImportExportFileInfo};
use crate::service::sync_service::SyncProgress;
use crate::service::usage_service::{UsageItemMetric, UsageMetrics};
use crate::service::{
    account_service, billing_service, db_state_service, drawing_service, file_service,
    path_service, sync_service, usage_service,
};
use crate::sync_service::WorkCalculated;

pub fn init_logger(log_path: &Path) -> Result<(), UnexpectedError> {
    log_service::init(log_path).map_err(|err| unexpected_only!("{:#?}", err))
}

pub fn get_db_state(config: &Config) -> Result<State, UnexpectedError> {
    db_state_service::get_state(config).map_err(|e| unexpected_only!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum MigrationError {
    StateRequiresCleaning,
}

pub fn migrate_db(config: &Config) -> Result<(), Error<MigrationError>> {
    db_state_service::perform_migration(config).map_err(|e| match e {
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
    account_service::create_account(config, username, api_url).map_err(|e| match e {
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
    account_service::import_account(config, account_string).map_err(|e| match e {
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
    account_service::export_account(config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(AccountExportError::NoAccount),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

pub fn get_account(config: &Config) -> Result<Account, Error<GetAccountError>> {
    account_repo::get(config).map_err(|e| match e {
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
) -> Result<DecryptedFileMetadata, Error<CreateFileAtPathError>> {
    path_service::create_at_path(config, path_and_name).map_err(|e| match e {
        CoreError::PathStartsWithNonRoot => UiError(CreateFileAtPathError::PathDoesntStartWithRoot),
        CoreError::PathContainsEmptyFileName => {
            UiError(CreateFileAtPathError::PathContainsEmptyFile)
        }
        CoreError::RootNonexistent => UiError(CreateFileAtPathError::NoRoot),
        CoreError::AccountNonexistent => UiError(CreateFileAtPathError::NoAccount),
        CoreError::PathTaken => UiError(CreateFileAtPathError::FileAlreadyExists),
        CoreError::FileNotFolder => UiError(CreateFileAtPathError::DocumentTreatedAsFolder),
        _ => unexpected!("{:#?}", e),
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
    file_service::write_document(config, id, content).map_err(|e| match e {
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
) -> Result<DecryptedFileMetadata, Error<CreateFileError>> {
    file_service::create_file(config, name, parent, file_type).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(CreateFileError::NoAccount),
        CoreError::FileNonexistent => UiError(CreateFileError::CouldNotFindAParent),
        CoreError::PathTaken => UiError(CreateFileError::FileNameNotAvailable),
        CoreError::FileNotFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
        CoreError::FileParentNonexistent => UiError(CreateFileError::CouldNotFindAParent),
        CoreError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
        CoreError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

pub fn get_root(config: &Config) -> Result<DecryptedFileMetadata, Error<GetRootError>> {
    file_service::get_root(config).map_err(|err| match err {
        CoreError::RootNonexistent => UiError(GetRootError::NoRoot),
        _ => unexpected!("{:#?}", err),
    })
}

pub fn get_children(
    config: &Config,
    id: Uuid,
) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
    file_service::get_children(config, id).map_err(|e| unexpected_only!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAndGetChildrenError {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
}

pub fn get_and_get_children_recursively(
    config: &Config,
    id: Uuid,
) -> Result<Vec<EncryptedFileMetadata>, Error<GetAndGetChildrenError>> {
    file_service::get_and_get_children_recursively(config, id).map_err(|e| match e {
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
) -> Result<DecryptedFileMetadata, Error<GetFileByIdError>> {
    file_service::get_not_deleted_metadata(config, RepoSource::Local, id).map_err(|e| match e {
        CoreError::FileNonexistent => UiError(GetFileByIdError::NoFileWithThatId),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

pub fn get_file_by_path(
    config: &Config,
    path: &str,
) -> Result<DecryptedFileMetadata, Error<GetFileByPathError>> {
    path_service::get_by_path(config, path).map_err(|e| match e {
        CoreError::FileNonexistent => UiError(GetFileByPathError::NoFileAtThatPath),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
}

pub fn delete_file(config: &Config, id: Uuid) -> Result<(), Error<FileDeleteError>> {
    file_service::delete_file(config, id).map_err(|e| match e {
        CoreError::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
        CoreError::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
        _ => unexpected!("{:#?}", e),
    })
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
    file_service::read_document(config, id).map_err(|e| match e {
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
    location: &str,
) -> Result<(), Error<SaveDocumentToDiskError>> {
    file_service::save_document_to_disk(config, id, location).map_err(|e| match e {
        CoreError::FileNotDocument => UiError(SaveDocumentToDiskError::TreatedFolderAsDocument),
        CoreError::AccountNonexistent => UiError(SaveDocumentToDiskError::NoAccount),
        CoreError::FileNonexistent => UiError(SaveDocumentToDiskError::FileDoesNotExist),
        CoreError::DiskPathInvalid => UiError(SaveDocumentToDiskError::BadPath),
        CoreError::DiskPathTaken => UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk),
        _ => unexpected!("{:#?}", e),
    })
}

pub fn list_paths(
    config: &Config,
    filter: Option<path_service::Filter>,
) -> Result<Vec<String>, UnexpectedError> {
    path_service::get_all_paths(config, filter).map_err(|e| unexpected_only!("{:#?}", e))
}

pub fn get_path_by_id(config: &Config, uuid: Uuid) -> Result<String, UnexpectedError> {
    path_service::get_path_by_id(config, uuid).map_err(|e| unexpected_only!("{:#?}", e))
}

pub fn list_metadatas(config: &Config) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
    file_service::get_all_not_deleted_metadata(config, RepoSource::Local)
        .map_err(|e| unexpected_only!("{:#?}", e))
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
    file_service::rename_file(config, id, new_name).map_err(|e| match e {
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
    file_service::move_file(config, id, new_parent).map_err(|e| match e {
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
    sync_service::sync(config, f).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(SyncAllError::NoAccount),
        CoreError::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

pub fn get_local_changes(config: &Config) -> Result<Vec<Uuid>, UnexpectedError> {
    file_service::get_local_changes(config).map_err(|e| unexpected_only!("{:#?}", e))
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, Error<CalculateWorkError>> {
    sync_service::calculate_work(config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(CalculateWorkError::NoAccount),
        CoreError::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

pub fn get_last_synced(config: &Config) -> Result<i64, UnexpectedError> {
    last_updated_repo::get(config).map_err(|e| unexpected_only!("{:#?}", e))
}

pub fn get_last_synced_human_string(config: &Config) -> Result<String, UnexpectedError> {
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
    usage_service::get_usage(config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(GetUsageError::NoAccount),
        CoreError::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
        CoreError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
        _ => unexpected!("{:#?}", e),
    })
}

pub fn get_uncompressed_usage(config: &Config) -> Result<UsageItemMetric, Error<GetUsageError>> {
    usage_service::get_uncompressed_usage(config).map_err(|e| match e {
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
    drawing_service::get_drawing(config, id).map_err(|e| match e {
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
    drawing_service::save_drawing(config, id, drawing_bytes).map_err(|e| match e {
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
    drawing_service::export_drawing(config, id, format, render_theme).map_err(|e| match e {
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
    location: &str,
) -> Result<(), Error<ExportDrawingToDiskError>> {
    drawing_service::export_drawing_to_disk(config, id, format, render_theme, location).map_err(
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

#[derive(Debug, Serialize, EnumIter)]
pub enum ImportFileError {
    NoAccount,
    ParentDoesNotExist,
    DocumentTreatedAsFolder,
    DiskPathInvalid,
}

pub fn import_file(
    config: &Config,
    disk_path: PathBuf,
    parent: Uuid,
    import_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), Error<ImportFileError>> {
    import_export_service::import_file(config, disk_path, parent, import_progress).map_err(|e| {
        match e {
            CoreError::AccountNonexistent => UiError(ImportFileError::NoAccount),
            CoreError::FileNonexistent => UiError(ImportFileError::ParentDoesNotExist),
            CoreError::FileNotFolder => UiError(ImportFileError::DocumentTreatedAsFolder),
            CoreError::DiskPathInvalid => UiError(ImportFileError::DiskPathInvalid),
            _ => unexpected!("{:#?}", e),
        }
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportFileError {
    NoAccount,
    ParentDoesNotExist,
    DiskPathTaken,
    DiskPathInvalid,
}

pub fn export_file(
    config: &Config,
    id: Uuid,
    destination: PathBuf,
    edit: bool,
    export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), Error<ExportFileError>> {
    import_export_service::export_file(config, id, destination, edit, export_progress).map_err(
        |e| match e {
            CoreError::AccountNonexistent => UiError(ExportFileError::NoAccount),
            CoreError::FileNonexistent => UiError(ExportFileError::ParentDoesNotExist),
            CoreError::DiskPathInvalid => UiError(ExportFileError::DiskPathInvalid),
            CoreError::DiskPathTaken => UiError(ExportFileError::DiskPathTaken),
            _ => unexpected!("{:#?}", e),
        },
    )
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SwitchAccountTierError {
    NoAccount,
    CouldNotReachServer,
    PreexistingCardDoesNotExist,
    NewTierIsOldTier,
    InvalidCreditCardNumber,
    InvalidCreditCardCVC,
    InvalidCreditCardExpYear,
    InvalidCreditCardExpMonth,
    CardDecline
}

pub fn switch_account_tier(
    config: &Config,
    new_account_tier: AccountTier,
) -> Result<(), Error<SwitchAccountTierError>> {
    billing_service::switch_account_tier(config, new_account_tier).map_err(|e| match e {
        CoreError::PreexistingCardDoesNotExist => {
            UiError(SwitchAccountTierError::PreexistingCardDoesNotExist)
        }
        CoreError::InvalidCreditCard(field) => match field {
            InvalidCreditCardType::Number => {
                UiError(SwitchAccountTierError::InvalidCreditCardNumber)
            }
            InvalidCreditCardType::ExpYear => {
                UiError(SwitchAccountTierError::InvalidCreditCardExpYear)
            }
            InvalidCreditCardType::ExpMonth => {
                UiError(SwitchAccountTierError::InvalidCreditCardExpMonth)
            }
            InvalidCreditCardType::CVC => UiError(SwitchAccountTierError::InvalidCreditCardCVC),
        },
        CoreError::NewTierIsOldTier => UiError(SwitchAccountTierError::NewTierIsOldTier),
        CoreError::ServerUnreachable => UiError(SwitchAccountTierError::CouldNotReachServer),
        CoreError::CardDecline(_) => UiError(SwitchAccountTierError::CardDecline),
        _ => unexpected!("{:#?}", e),
    })
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetLastRegisteredCardError {
    NoAccount,
    CouldNotReachServer,
}

pub fn get_last_registered_card(
    config: &Config,
) -> Result<CreditCardInfo, Error<GetLastRegisteredCardError>> {
    billing_service::get_last_registered_card(config).map_err(|e| match e {
        CoreError::AccountNonexistent => UiError(GetLastRegisteredCardError::NoAccount),
        CoreError::ServerUnreachable => UiError(GetLastRegisteredCardError::CouldNotReachServer),
        _ => unexpected!("{:#?}", e),
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
    MigrationError,
    CreateAccountError,
    ImportError,
    AccountExportError,
    GetAccountError,
    CreateFileAtPathError,
    WriteToDocumentError,
    CreateFileError,
    GetRootError,
    GetFileByIdError,
    GetFileByPathError,
    FileDeleteError,
    ReadDocumentError,
    RenameFileError,
    MoveFileError,
    SyncAllError,
    CalculateWorkError,
    GetUsageError,
    GetDrawingError,
    SaveDrawingError,
    ExportDrawingError,
    ExportDrawingToDiskError,
    SaveDocumentToDiskError,
);

pub mod external_interface;
pub mod model;
pub mod pure_functions;
pub mod repo;
pub mod service;

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
