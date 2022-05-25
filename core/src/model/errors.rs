use std::fmt::{Display, Formatter};
use std::io::ErrorKind;

use lockbook_models::api;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use strum_macros::EnumIter;
use uuid::Uuid;

use lockbook_models::api::{GetPublicKeyError, GetUpdatesError, NewAccountError};
use lockbook_models::tree::TreeError;

use crate::service::api_service::ApiError;
use crate::UiError;

#[derive(Debug)]
pub struct UnexpectedError(pub String);

impl Display for UnexpectedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected error: {}", self.0)
    }
}

impl From<CoreError> for UnexpectedError {
    fn from(e: CoreError) -> Self {
        UnexpectedError(format!("{:?}", e))
    }
}

impl From<hmdb::errors::Error> for UnexpectedError {
    fn from(err: hmdb::errors::Error) -> Self {
        UnexpectedError(format!("{:#?}", err))
    }
}

impl Serialize for UnexpectedError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UnexpectedError", 2)?;
        state.serialize_field("tag", "Unexpected")?;
        state.serialize_field("content", &self.0)?;
        state.end()
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error<U: Serialize> {
    UiError(U),
    Unexpected(String),
}

#[macro_export]
macro_rules! unexpected {
    ($base:literal $(, $args:tt )*) => {{
        debug!($base $(, $args )*);
        debug!("{:?}", backtrace::Backtrace::new());
        debug!($base $(, $args )*);
        Error::Unexpected(format!($base $(, $args )*))
    }};
}

#[macro_export]
macro_rules! unexpected_only {
    ($base:literal $(, $args:tt )*) => {{
        debug!($base $(, $args )*);
        debug!("{:?}", backtrace::Backtrace::new());
        debug!($base $(, $args )*);
        UnexpectedError(format!($base $(, $args )*))
    }};
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    AccountExists,
    AccountNonexistent,
    AccountStringCorrupted,
    AlreadyPremium,
    ConcurrentRequestsAreTooSoon,
    CardDecline,
    CardHasInsufficientFunds,
    TryAgain,
    CardNotSupported,
    ExpiredCard,
    ClientUpdateRequired,
    ClientWipeRequired,
    CurrentUsageIsMoreThanNewTier,
    DiskPathInvalid,
    DiskPathTaken,
    ServerDisabled,
    DrawingInvalid,
    FileExists,
    FileNameContainsSlash,
    FileNameEmpty,
    FileNonexistent,
    FileNotDocument,
    FileNotFolder,
    FileParentNonexistent,
    FolderMovedIntoSelf,
    InvalidCardNumber,
    InvalidCardExpYear,
    InvalidCardExpMonth,
    InvalidCardCvc,
    InvalidPurchaseToken,
    NewTierIsOldTier,
    NoCardAdded,
    NotPremium,
    PathContainsEmptyFileName,
    PathNonexistent,
    PathStartsWithNonRoot,
    PathTaken,
    OldCardDoesNotExist,
    RootModificationInvalid,
    RootNonexistent,
    ServerUnreachable,
    UsageIsOverFreeTierDataCap,
    UsernameInvalid,
    UsernamePublicKeyMismatch,
    UsernameTaken,
    Unexpected(String),
}

pub fn core_err_unexpected<T: std::fmt::Debug>(err: T) -> CoreError {
    CoreError::Unexpected(format!("{:#?}", err))
}

impl From<hmdb::errors::Error> for CoreError {
    fn from(err: hmdb::errors::Error) -> Self {
        core_err_unexpected(err)
    }
}

impl<E: Serialize> From<hmdb::errors::Error> for Error<E> {
    fn from(err: hmdb::errors::Error) -> Self {
        Self::Unexpected(format!("{:#?}", err))
    }
}

impl From<TreeError> for CoreError {
    fn from(tree: TreeError) -> Self {
        match tree {
            TreeError::FileNonexistent => CoreError::FileNonexistent,
            TreeError::FileParentNonexistent => CoreError::FileParentNonexistent,
            TreeError::RootNonexistent => CoreError::RootNonexistent,
            TreeError::Unexpected(err) => CoreError::Unexpected(err),
        }
    }
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

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateAccountError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
    ServerDisabled,
}

impl From<CoreError> for Error<CreateAccountError> {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::AccountExists => UiError(CreateAccountError::AccountExistsAlready),
            CoreError::UsernameTaken => UiError(CreateAccountError::UsernameTaken),
            CoreError::UsernameInvalid => UiError(CreateAccountError::InvalidUsername),
            CoreError::ServerUnreachable => UiError(CreateAccountError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CreateAccountError::ClientUpdateRequired),
            CoreError::ServerDisabled => UiError(CreateAccountError::ServerDisabled),
            _ => unexpected!("{:#?}", err),
        }
    }
}

impl From<ApiError<NewAccountError>> for CoreError {
    fn from(err: ApiError<NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(NewAccountError::UsernameTaken) => CoreError::UsernameTaken,
            ApiError::Endpoint(NewAccountError::InvalidUsername) => CoreError::UsernameInvalid,
            ApiError::Endpoint(NewAccountError::Disabled) => CoreError::ServerDisabled,
            e => core_err_unexpected(e),
        }
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

impl From<ApiError<GetPublicKeyError>> for CoreError {
    fn from(err: ApiError<GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(GetPublicKeyError::UserNotFound) => CoreError::AccountNonexistent,
            e => core_err_unexpected(e),
        }
    }
}

impl From<CoreError> for Error<ImportError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountStringCorrupted => UiError(ImportError::AccountStringCorrupted),
            CoreError::AccountExists => UiError(ImportError::AccountExistsAlready),
            CoreError::UsernamePublicKeyMismatch => UiError(ImportError::UsernamePKMismatch),
            CoreError::ServerUnreachable => UiError(ImportError::CouldNotReachServer),
            CoreError::AccountNonexistent => UiError(ImportError::AccountDoesNotExist),
            CoreError::ClientUpdateRequired => UiError(ImportError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AccountExportError {
    NoAccount,
}

impl From<CoreError> for Error<AccountExportError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(AccountExportError::NoAccount),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

impl From<CoreError> for Error<GetAccountError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(GetAccountError::NoAccount),
            _ => unexpected!("{:#?}", e),
        }
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

impl From<CoreError> for Error<CreateFileAtPathError> {
    fn from(e: CoreError) -> Self {
        match e {
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
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

impl From<CoreError> for Error<GetFileByPathError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(GetFileByPathError::NoFileAtThatPath),
            _ => unexpected!("{:#?}", e),
        }
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

impl From<CoreError> for Error<CreateFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(CreateFileError::NoAccount),
            CoreError::FileNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            CoreError::PathTaken => UiError(CreateFileError::FileNameNotAvailable),
            CoreError::FileNotFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
            CoreError::FileParentNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            CoreError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
            CoreError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum WriteToDocumentError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDocument,
}

impl From<CoreError> for Error<WriteToDocumentError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(WriteToDocumentError::NoAccount),
            CoreError::FileNonexistent => UiError(WriteToDocumentError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(WriteToDocumentError::FolderTreatedAsDocument),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

impl From<CoreError> for Error<GetRootError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootNonexistent => UiError(GetRootError::NoRoot),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAndGetChildrenError {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
}

impl From<CoreError> for Error<GetAndGetChildrenError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(GetAndGetChildrenError::FileDoesNotExist),
            CoreError::FileNotFolder => UiError(GetAndGetChildrenError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByIdError {
    NoFileWithThatId,
}

impl From<CoreError> for Error<GetFileByIdError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(GetFileByIdError::NoFileWithThatId),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
}

impl From<CoreError> for Error<FileDeleteError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
            CoreError::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ReadDocumentError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
}

impl From<CoreError> for Error<ReadDocumentError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNotDocument => UiError(ReadDocumentError::TreatedFolderAsDocument),
            CoreError::AccountNonexistent => UiError(ReadDocumentError::NoAccount),
            CoreError::FileNonexistent => UiError(ReadDocumentError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDocumentToDiskError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
    BadPath,
    FileAlreadyExistsInDisk,
}

impl From<CoreError> for Error<SaveDocumentToDiskError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNotDocument => UiError(SaveDocumentToDiskError::TreatedFolderAsDocument),
            CoreError::AccountNonexistent => UiError(SaveDocumentToDiskError::NoAccount),
            CoreError::FileNonexistent => UiError(SaveDocumentToDiskError::FileDoesNotExist),
            CoreError::DiskPathInvalid => UiError(SaveDocumentToDiskError::BadPath),
            CoreError::DiskPathTaken => UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk),
            _ => unexpected!("{:#?}", e),
        }
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

impl From<CoreError> for Error<RenameFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(RenameFileError::FileDoesNotExist),
            CoreError::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
            CoreError::FileNameContainsSlash => UiError(RenameFileError::NewNameContainsSlash),
            CoreError::PathTaken => UiError(RenameFileError::FileNameNotAvailable),
            CoreError::RootModificationInvalid => UiError(RenameFileError::CannotRenameRoot),
            _ => unexpected!("{:#?}", e),
        }
    }
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

impl From<CoreError> for Error<MoveFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootModificationInvalid => UiError(MoveFileError::CannotMoveRoot),
            CoreError::FileNotFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
            CoreError::FileNonexistent => UiError(MoveFileError::FileDoesNotExist),
            CoreError::FolderMovedIntoSelf => UiError(MoveFileError::FolderMovedIntoItself),
            CoreError::AccountNonexistent => UiError(MoveFileError::NoAccount),
            CoreError::FileParentNonexistent => UiError(MoveFileError::TargetParentDoesNotExist),
            CoreError::PathTaken => UiError(MoveFileError::TargetParentHasChildNamedThat),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SyncAllError {
    NoAccount,
    ClientUpdateRequired,
    CouldNotReachServer,
}

impl From<CoreError> for Error<SyncAllError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(SyncAllError::NoAccount),
            CoreError::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

impl From<ApiError<GetUpdatesError>> for CoreError {
    fn from(e: ApiError<GetUpdatesError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::GetDocumentError>> for CoreError {
    fn from(e: ApiError<api::GetDocumentError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::FileMetadataUpsertsError>> for CoreError {
    fn from(e: ApiError<api::FileMetadataUpsertsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::ChangeDocumentContentError>> for CoreError {
    fn from(e: ApiError<api::ChangeDocumentContentError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<CalculateWorkError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(CalculateWorkError::NoAccount),
            CoreError::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetUsageError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<GetUsageError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(GetUsageError::NoAccount),
            CoreError::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

impl From<ApiError<api::GetUsageError>> for CoreError {
    fn from(e: ApiError<api::GetUsageError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetDrawingError {
    NoAccount,
    FolderTreatedAsDrawing,
    InvalidDrawing,
    FileDoesNotExist,
}

impl From<CoreError> for Error<GetDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(GetDrawingError::InvalidDrawing),
            CoreError::FileNotDocument => UiError(GetDrawingError::FolderTreatedAsDrawing),
            CoreError::AccountNonexistent => UiError(GetDrawingError::NoAccount),
            CoreError::FileNonexistent => UiError(GetDrawingError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDrawingError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDrawing,
    InvalidDrawing,
}

impl From<CoreError> for Error<SaveDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(SaveDrawingError::InvalidDrawing),
            CoreError::AccountNonexistent => UiError(SaveDrawingError::NoAccount),
            CoreError::FileNonexistent => UiError(SaveDrawingError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(SaveDrawingError::FolderTreatedAsDrawing),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportDrawingError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    NoAccount,
    InvalidDrawing,
}

impl From<CoreError> for Error<ExportDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(ExportDrawingError::InvalidDrawing),
            CoreError::AccountNonexistent => UiError(ExportDrawingError::NoAccount),
            CoreError::FileNonexistent => UiError(ExportDrawingError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(ExportDrawingError::FolderTreatedAsDrawing),
            _ => unexpected!("{:#?}", e),
        }
    }
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

impl From<CoreError> for Error<ExportDrawingToDiskError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(ExportDrawingToDiskError::InvalidDrawing),
            CoreError::AccountNonexistent => UiError(ExportDrawingToDiskError::NoAccount),
            CoreError::FileNonexistent => UiError(ExportDrawingToDiskError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(ExportDrawingToDiskError::FolderTreatedAsDrawing),
            CoreError::DiskPathInvalid => UiError(ExportDrawingToDiskError::BadPath),
            CoreError::DiskPathTaken => UiError(ExportDrawingToDiskError::FileAlreadyExistsInDisk),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ImportFileError {
    NoAccount,
    ParentDoesNotExist,
    DocumentTreatedAsFolder,
}

impl From<CoreError> for Error<ImportFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(ImportFileError::NoAccount),
            CoreError::FileNonexistent => UiError(ImportFileError::ParentDoesNotExist),
            CoreError::FileNotFolder => UiError(ImportFileError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportFileError {
    NoAccount,
    ParentDoesNotExist,
    DiskPathTaken,
    DiskPathInvalid,
}

impl From<CoreError> for Error<ExportFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(ExportFileError::NoAccount),
            CoreError::FileNonexistent => UiError(ExportFileError::ParentDoesNotExist),
            CoreError::DiskPathInvalid => UiError(ExportFileError::DiskPathInvalid),
            CoreError::DiskPathTaken => UiError(ExportFileError::DiskPathTaken),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum UpgradeAccountStripeError {
    NoAccount,
    CouldNotReachServer,
    OldCardDoesNotExist,
    NewTierIsOldTier,
    InvalidCardNumber,
    InvalidCardCvc,
    InvalidCardExpYear,
    InvalidCardExpMonth,
    CardDecline,
    CardHasInsufficientFunds,
    TryAgain,
    CardNotSupported,
    ExpiredCard,
    ClientUpdateRequired,
    CurrentUsageIsMoreThanNewTier,
    ConcurrentRequestsAreTooSoon,
}

impl From<CoreError> for Error<UpgradeAccountStripeError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::OldCardDoesNotExist => {
                UiError(UpgradeAccountStripeError::OldCardDoesNotExist)
            }
            CoreError::InvalidCardNumber => UiError(UpgradeAccountStripeError::InvalidCardNumber),
            CoreError::InvalidCardExpYear => UiError(UpgradeAccountStripeError::InvalidCardExpYear),
            CoreError::InvalidCardExpMonth => {
                UiError(UpgradeAccountStripeError::InvalidCardExpMonth)
            }
            CoreError::InvalidCardCvc => UiError(UpgradeAccountStripeError::InvalidCardCvc),
            CoreError::NewTierIsOldTier => UiError(UpgradeAccountStripeError::NewTierIsOldTier),
            CoreError::ServerUnreachable => UiError(UpgradeAccountStripeError::CouldNotReachServer),
            CoreError::CardDecline => UiError(UpgradeAccountStripeError::CardDecline),
            CoreError::CardHasInsufficientFunds => {
                UiError(UpgradeAccountStripeError::CardHasInsufficientFunds)
            }
            CoreError::TryAgain => UiError(UpgradeAccountStripeError::TryAgain),
            CoreError::CardNotSupported => UiError(UpgradeAccountStripeError::CardNotSupported),
            CoreError::ExpiredCard => UiError(UpgradeAccountStripeError::ExpiredCard),
            CoreError::CurrentUsageIsMoreThanNewTier => {
                UiError(UpgradeAccountStripeError::CurrentUsageIsMoreThanNewTier)
            }
            CoreError::AccountNonexistent => UiError(UpgradeAccountStripeError::NoAccount),
            CoreError::ConcurrentRequestsAreTooSoon => {
                UiError(UpgradeAccountStripeError::ConcurrentRequestsAreTooSoon)
            }
            CoreError::ClientUpdateRequired => {
                UiError(UpgradeAccountStripeError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetCreditCard {
    NoAccount,
    CouldNotReachServer,
    NoCardAdded,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<GetCreditCard> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AccountNonexistent => UiError(GetCreditCard::NoAccount),
            CoreError::ServerUnreachable => UiError(GetCreditCard::CouldNotReachServer),
            CoreError::NoCardAdded => UiError(GetCreditCard::NoCardAdded),
            CoreError::ClientUpdateRequired => UiError(GetCreditCard::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ConfirmAndroidSubscriptionError {
    AlreadyPremium,
    InvalidPurchaseToken,
    ConcurrentRequestsAreTooSoon,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<ConfirmAndroidSubscriptionError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AlreadyPremium => UiError(ConfirmAndroidSubscriptionError::AlreadyPremium),
            CoreError::InvalidPurchaseToken => {
                UiError(ConfirmAndroidSubscriptionError::InvalidPurchaseToken)
            }
            CoreError::ConcurrentRequestsAreTooSoon => {
                UiError(ConfirmAndroidSubscriptionError::ConcurrentRequestsAreTooSoon)
            }
            CoreError::ServerUnreachable => UiError(ConfirmAndroidSubscriptionError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(ConfirmAndroidSubscriptionError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CancelSubscriptionError {
    NotPremium,
    UsageIsOverFreeTierDataCap,
    ConcurrentRequestsAreTooSoon,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<CancelSubscriptionError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NotPremium => UiError(CancelSubscriptionError::NotPremium),
            CoreError::UsageIsOverFreeTierDataCap => {
                UiError(CancelSubscriptionError::UsageIsOverFreeTierDataCap)
            }
            CoreError::ConcurrentRequestsAreTooSoon => {
                UiError(CancelSubscriptionError::ConcurrentRequestsAreTooSoon)
            }
            CoreError::ServerUnreachable => UiError(CancelSubscriptionError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CancelSubscriptionError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetSubscriptionInfoError {
    NotPremium,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<GetSubscriptionInfoError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NotPremium => UiError(GetSubscriptionInfoError::NotPremium),
            CoreError::ServerUnreachable => UiError(GetSubscriptionInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(GetSubscriptionInfoError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TestRepoError {
    NoAccount,
    NoRootFolder,
    DocumentTreatedAsFolder(Uuid),
    FileOrphaned(Uuid),
    CycleDetected(Uuid),
    FileNameEmpty(Uuid),
    FileNameContainsSlash(Uuid),
    NameConflictDetected(Uuid),
    DocumentReadError(Uuid, CoreError),
    Tree(TreeError),
    Core(CoreError),
}

impl From<CoreError> for TestRepoError {
    fn from(e: CoreError) -> Self {
        Self::Core(e)
    }
}

#[derive(Debug, Clone)]
pub enum Warning {
    EmptyFile(Uuid),
    InvalidUTF8(Uuid),
    UnreadableDrawing(Uuid),
}
