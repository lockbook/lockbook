use std::collections::HashSet;
use std::fmt;
use std::io;
use std::sync::PoisonError;

use itertools::Itertools;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use strum_macros::EnumIter;
use uuid::Uuid;

use lockbook_shared::api::{
    self, GetFileIdsError, GetPublicKeyError, GetUpdatesError, GetUsernameError, NewAccountError,
};
use lockbook_shared::{SharedError, ValidationFailure};

use crate::service::api_service::ApiError;
use crate::UiError;

#[derive(Debug)]
pub struct UnexpectedError(pub String);

impl fmt::Display for UnexpectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
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

impl<T> From<std::sync::PoisonError<T>> for UnexpectedError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        UnexpectedError(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvError) -> Self {
        UnexpectedError(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvTimeoutError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvTimeoutError) -> Self {
        UnexpectedError(format!("{:#?}", err))
    }
}

impl<T> From<crossbeam::channel::SendError<T>> for UnexpectedError {
    fn from(err: crossbeam::channel::SendError<T>) -> Self {
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

impl From<UnexpectedError> for String {
    fn from(v: UnexpectedError) -> Self {
        v.0
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

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    AccountExists,
    AccountNonexistent,
    AccountStringCorrupted,
    AlreadyCanceled,
    AlreadyPremium,
    AppStoreAccountAlreadyLinked,
    CannotCancelSubscriptionForAppStore,
    CardDecline,
    CardHasInsufficientFunds,
    CardNotSupported,
    ClientUpdateRequired,
    ClientWipeRequired,
    CurrentUsageIsMoreThanNewTier,
    DiskPathInvalid,
    DiskPathTaken,
    DrawingInvalid,
    ExistingRequestPending,
    ExpiredCard,
    FileExists,
    FileIsLink,
    FileNotShared,
    FileNameContainsSlash,
    FileNameTooLong,
    FileNameEmpty,
    FileNonexistent,
    FileNotDocument,
    FileNotFolder,
    FileParentNonexistent,
    FolderMovedIntoSelf,
    InsufficientPermission,
    InvalidCardCvc,
    InvalidCardExpMonth,
    InvalidCardExpYear,
    InvalidCardNumber,
    InvalidPurchaseToken,
    InvalidAuthDetails,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    MultipleLinksToSameFile,
    NotPremium,
    OldCardDoesNotExist,
    PathContainsEmptyFileName,
    PathNonexistent,
    PathStartsWithNonRoot,
    PathTaken,
    RootModificationInvalid,
    RootNonexistent,
    ServerDisabled,
    ServerUnreachable,
    ShareAlreadyExists,
    ShareNonexistent,
    TryAgain,
    UsageIsOverFreeTierDataCap,
    UsernameInvalid,
    UsernameNotFound,
    UsernamePublicKeyMismatch,
    UsernameTaken,
    Unexpected(String),
}

pub fn core_err_unexpected<T: fmt::Debug>(err: T) -> CoreError {
    CoreError::Unexpected(format!("{:#?}", err))
}

impl From<SharedError> for CoreError {
    fn from(err: SharedError) -> Self {
        match err {
            SharedError::RootNonexistent => Self::RootNonexistent,
            SharedError::FileNonexistent => Self::FileNonexistent,
            SharedError::FileParentNonexistent => Self::FileParentNonexistent,
            SharedError::Unexpected(err) => Self::Unexpected(err.to_string()),
            SharedError::PathContainsEmptyFileName => Self::PathContainsEmptyFileName,
            SharedError::PathTaken => Self::PathTaken,
            SharedError::FileNameContainsSlash => Self::FileNameContainsSlash,
            SharedError::RootModificationInvalid => Self::RootModificationInvalid,
            SharedError::DeletedFileUpdated(_) => Self::FileNonexistent,
            SharedError::FileNameEmpty => Self::FileNameEmpty,
            SharedError::FileNotFolder => Self::FileNotFolder,
            SharedError::FileNotDocument => Self::FileNotDocument,
            SharedError::InsufficientPermission => Self::InsufficientPermission,
            SharedError::NotPermissioned => Self::InsufficientPermission,
            SharedError::ShareNonexistent => Self::ShareNonexistent,
            SharedError::DuplicateShare => Self::ShareAlreadyExists,
            SharedError::ValidationFailure(failure) => match failure {
                ValidationFailure::Cycle(_) => Self::FolderMovedIntoSelf,
                ValidationFailure::PathConflict(_) => Self::PathTaken,
                ValidationFailure::SharedLink { .. } => Self::LinkInSharedFolder,
                ValidationFailure::DuplicateLink { .. } => Self::MultipleLinksToSameFile,
                ValidationFailure::BrokenLink(_) => Self::LinkTargetNonexistent,
                ValidationFailure::OwnedLink(_) => Self::LinkTargetIsOwned,
                ValidationFailure::NonFolderWithChildren(_) => Self::FileNotFolder,
                vf => Self::Unexpected(format!("unexpected validation failure {:?}", vf)),
            },
            _ => Self::Unexpected(format!("unexpected shared error {:?}", err)),
        }
    }
}

impl From<db_rs::DbError> for CoreError {
    fn from(err: db_rs::DbError) -> Self {
        core_err_unexpected(err)
    }
}

impl<G> From<PoisonError<G>> for CoreError {
    fn from(err: PoisonError<G>) -> Self {
        core_err_unexpected(err)
    }
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

impl From<io::Error> for CoreError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound
            | io::ErrorKind::PermissionDenied
            | io::ErrorKind::InvalidInput => Self::DiskPathInvalid,
            io::ErrorKind::AlreadyExists => Self::DiskPathTaken,
            _ => core_err_unexpected(e),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        Self::Unexpected(format!("{err}"))
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

impl From<ApiError<GetUsernameError>> for CoreError {
    fn from(err: ApiError<GetUsernameError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(GetUsernameError::UserNotFound) => CoreError::AccountNonexistent,
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
    NoRoot,
    PathContainsEmptyFile,
    DocumentTreatedAsFolder,
    InsufficientPermission,
}

impl From<CoreError> for Error<CreateFileAtPathError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::PathContainsEmptyFileName => {
                UiError(CreateFileAtPathError::PathContainsEmptyFile)
            }
            CoreError::RootNonexistent => UiError(CreateFileAtPathError::NoRoot),
            CoreError::PathTaken => UiError(CreateFileAtPathError::FileAlreadyExists),
            CoreError::FileNotFolder => UiError(CreateFileAtPathError::DocumentTreatedAsFolder),
            CoreError::InsufficientPermission => {
                UiError(CreateFileAtPathError::InsufficientPermission)
            }
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
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameEmpty,
    FileNameTooLong,
    FileNameContainsSlash,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    InsufficientPermission,
    MultipleLinksToSameFile,
}

impl From<CoreError> for Error<CreateFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            CoreError::PathTaken => UiError(CreateFileError::FileNameNotAvailable),
            CoreError::FileNotFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
            CoreError::FileParentNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            CoreError::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
            CoreError::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
            CoreError::LinkInSharedFolder => UiError(CreateFileError::LinkInSharedFolder),
            CoreError::LinkTargetIsOwned => UiError(CreateFileError::LinkTargetIsOwned),
            CoreError::LinkTargetNonexistent => UiError(CreateFileError::LinkTargetNonexistent),
            CoreError::InsufficientPermission => UiError(CreateFileError::InsufficientPermission),
            CoreError::MultipleLinksToSameFile => UiError(CreateFileError::MultipleLinksToSameFile),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum WriteToDocumentError {
    FileDoesNotExist,
    FolderTreatedAsDocument,
    InsufficientPermission,
}

impl From<CoreError> for Error<WriteToDocumentError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(WriteToDocumentError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(WriteToDocumentError::FolderTreatedAsDocument),
            CoreError::InsufficientPermission => {
                UiError(WriteToDocumentError::InsufficientPermission)
            }
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
    InsufficientPermission,
}

impl From<CoreError> for Error<FileDeleteError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
            CoreError::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
            CoreError::InsufficientPermission => UiError(FileDeleteError::InsufficientPermission),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ReadDocumentError {
    TreatedFolderAsDocument,
    FileDoesNotExist,
}

impl From<CoreError> for Error<ReadDocumentError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNotDocument => UiError(ReadDocumentError::TreatedFolderAsDocument),
            CoreError::FileNonexistent => UiError(ReadDocumentError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDocumentToDiskError {
    TreatedFolderAsDocument,
    FileDoesNotExist,
    BadPath,
    FileAlreadyExistsInDisk,
}

impl From<CoreError> for Error<SaveDocumentToDiskError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNotDocument => UiError(SaveDocumentToDiskError::TreatedFolderAsDocument),
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
    FileNameTooLong,
    NewNameContainsSlash,
    FileNameNotAvailable,
    CannotRenameRoot,
    InsufficientPermission,
}

impl From<CoreError> for Error<RenameFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(RenameFileError::FileDoesNotExist),
            CoreError::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
            CoreError::FileNameContainsSlash => UiError(RenameFileError::NewNameContainsSlash),
            CoreError::PathTaken => UiError(RenameFileError::FileNameNotAvailable),
            CoreError::RootModificationInvalid => UiError(RenameFileError::CannotRenameRoot),
            CoreError::InsufficientPermission => UiError(RenameFileError::InsufficientPermission),
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
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
    LinkInSharedFolder,
    InsufficientPermission,
}

impl From<CoreError> for Error<MoveFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootModificationInvalid => UiError(MoveFileError::CannotMoveRoot),
            CoreError::FileNotFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
            CoreError::FileNonexistent => UiError(MoveFileError::FileDoesNotExist),
            CoreError::FolderMovedIntoSelf => UiError(MoveFileError::FolderMovedIntoItself),
            CoreError::FileParentNonexistent => UiError(MoveFileError::TargetParentDoesNotExist),
            CoreError::PathTaken => UiError(MoveFileError::TargetParentHasChildNamedThat),
            CoreError::LinkInSharedFolder => UiError(MoveFileError::LinkInSharedFolder),
            CoreError::InsufficientPermission => UiError(MoveFileError::InsufficientPermission),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ShareFileError {
    CannotShareRoot,
    FileNonexistent,
    ShareAlreadyExists,
    LinkInSharedFolder,
    InsufficientPermission,
}

impl From<CoreError> for Error<ShareFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::RootModificationInvalid => UiError(ShareFileError::CannotShareRoot),
            CoreError::FileNonexistent => UiError(ShareFileError::FileNonexistent),
            CoreError::ShareAlreadyExists => UiError(ShareFileError::ShareAlreadyExists),
            CoreError::LinkInSharedFolder => UiError(ShareFileError::LinkInSharedFolder),
            CoreError::InsufficientPermission => UiError(ShareFileError::InsufficientPermission),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum DeletePendingShareError {
    FileNonexistent,
    ShareNonexistent,
}

impl From<CoreError> for Error<DeletePendingShareError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(DeletePendingShareError::FileNonexistent),
            CoreError::ShareNonexistent => UiError(DeletePendingShareError::ShareNonexistent),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateLinkAtPathError {
    FileAlreadyExists,
    PathContainsEmptyFile,
    DocumentTreatedAsFolder,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    MultipleLinksToSameFile,
}

impl From<CoreError> for Error<CreateLinkAtPathError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::PathContainsEmptyFileName => {
                UiError(CreateLinkAtPathError::PathContainsEmptyFile)
            }
            CoreError::PathTaken => UiError(CreateLinkAtPathError::FileAlreadyExists),
            CoreError::FileNotFolder => UiError(CreateLinkAtPathError::DocumentTreatedAsFolder),
            CoreError::LinkInSharedFolder => UiError(CreateLinkAtPathError::LinkInSharedFolder),
            CoreError::LinkTargetIsOwned => UiError(CreateLinkAtPathError::LinkTargetIsOwned),
            CoreError::MultipleLinksToSameFile => {
                UiError(CreateLinkAtPathError::MultipleLinksToSameFile)
            }
            CoreError::LinkTargetNonexistent => {
                UiError(CreateLinkAtPathError::LinkTargetNonexistent)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SyncAllError {
    Retry,
    ClientUpdateRequired,
    CouldNotReachServer,
}

impl From<CoreError> for Error<SyncAllError> {
    fn from(e: CoreError) -> Self {
        match e {
            // TODO figure out under what circumstances a user should retry a sync
            CoreError::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

impl From<ApiError<GetFileIdsError>> for CoreError {
    fn from(e: ApiError<GetFileIdsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
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

impl From<ApiError<api::UpsertError>> for CoreError {
    fn from(e: ApiError<api::UpsertError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::ChangeDocError>> for CoreError {
    fn from(e: ApiError<api::ChangeDocError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<CalculateWorkError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetUsageError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<GetUsageError> {
    fn from(e: CoreError) -> Self {
        match e {
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
    FolderTreatedAsDrawing,
    InvalidDrawing,
    FileDoesNotExist,
}

impl From<CoreError> for Error<GetDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(GetDrawingError::InvalidDrawing),
            CoreError::FileNotDocument => UiError(GetDrawingError::FolderTreatedAsDrawing),
            CoreError::FileNonexistent => UiError(GetDrawingError::FileDoesNotExist),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDrawingError {
    FileDoesNotExist,
    FolderTreatedAsDrawing,
    InvalidDrawing,
}

impl From<CoreError> for Error<SaveDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(SaveDrawingError::InvalidDrawing),
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
    InvalidDrawing,
}

impl From<CoreError> for Error<ExportDrawingError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(ExportDrawingError::InvalidDrawing),
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
    InvalidDrawing,
    BadPath,
    FileAlreadyExistsInDisk,
}

impl From<CoreError> for Error<ExportDrawingToDiskError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DrawingInvalid => UiError(ExportDrawingToDiskError::InvalidDrawing),
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
    ParentDoesNotExist,
    DocumentTreatedAsFolder,
}

impl From<CoreError> for Error<ImportFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(ImportFileError::ParentDoesNotExist),
            CoreError::FileNotFolder => UiError(ImportFileError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportFileError {
    ParentDoesNotExist,
    DiskPathTaken,
    DiskPathInvalid,
}

impl From<CoreError> for Error<ExportFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::FileNonexistent => UiError(ExportFileError::ParentDoesNotExist),
            CoreError::DiskPathInvalid => UiError(ExportFileError::DiskPathInvalid),
            CoreError::DiskPathTaken => UiError(ExportFileError::DiskPathTaken),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum UpgradeAccountStripeError {
    CouldNotReachServer,
    OldCardDoesNotExist,
    AlreadyPremium,
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
    ExistingRequestPending,
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
            CoreError::AlreadyPremium => UiError(UpgradeAccountStripeError::AlreadyPremium),
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
            CoreError::ExistingRequestPending => {
                UiError(UpgradeAccountStripeError::ExistingRequestPending)
            }
            CoreError::ClientUpdateRequired => {
                UiError(UpgradeAccountStripeError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum UpgradeAccountGooglePlayError {
    AppStoreAccountAlreadyLinked,
    AlreadyPremium,
    InvalidAuthDetails,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<UpgradeAccountGooglePlayError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AlreadyPremium => UiError(UpgradeAccountGooglePlayError::AlreadyPremium),
            CoreError::InvalidAuthDetails => {
                UiError(UpgradeAccountGooglePlayError::InvalidAuthDetails)
            }
            CoreError::ExistingRequestPending => {
                UiError(UpgradeAccountGooglePlayError::ExistingRequestPending)
            }
            CoreError::ServerUnreachable => {
                UiError(UpgradeAccountGooglePlayError::CouldNotReachServer)
            }
            CoreError::ClientUpdateRequired => {
                UiError(UpgradeAccountGooglePlayError::ClientUpdateRequired)
            }
            CoreError::AppStoreAccountAlreadyLinked => {
                UiError(UpgradeAccountGooglePlayError::AppStoreAccountAlreadyLinked)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum UpgradeAccountAppStoreError {
    AppStoreAccountAlreadyLinked,
    AlreadyPremium,
    InvalidPurchaseToken,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<UpgradeAccountAppStoreError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::AlreadyPremium => UiError(UpgradeAccountAppStoreError::AlreadyPremium),
            CoreError::InvalidPurchaseToken => {
                UiError(UpgradeAccountAppStoreError::InvalidPurchaseToken)
            }
            CoreError::ExistingRequestPending => {
                UiError(UpgradeAccountAppStoreError::ExistingRequestPending)
            }
            CoreError::ServerUnreachable => {
                UiError(UpgradeAccountAppStoreError::CouldNotReachServer)
            }
            CoreError::ClientUpdateRequired => {
                UiError(UpgradeAccountAppStoreError::ClientUpdateRequired)
            }
            CoreError::AppStoreAccountAlreadyLinked => {
                UiError(UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CancelSubscriptionError {
    NotPremium,
    AlreadyCanceled,
    UsageIsOverFreeTierDataCap,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired,
    CannotCancelForAppStore,
}

impl From<CoreError> for Error<CancelSubscriptionError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NotPremium => UiError(CancelSubscriptionError::NotPremium),
            CoreError::AlreadyCanceled => UiError(CancelSubscriptionError::AlreadyCanceled),
            CoreError::UsageIsOverFreeTierDataCap => {
                UiError(CancelSubscriptionError::UsageIsOverFreeTierDataCap)
            }
            CoreError::ExistingRequestPending => {
                UiError(CancelSubscriptionError::ExistingRequestPending)
            }
            CoreError::CannotCancelSubscriptionForAppStore => {
                UiError(CancelSubscriptionError::CannotCancelForAppStore)
            }
            CoreError::ServerUnreachable => UiError(CancelSubscriptionError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(CancelSubscriptionError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetSubscriptionInfoError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<GetSubscriptionInfoError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::ServerUnreachable => UiError(GetSubscriptionInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(GetSubscriptionInfoError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum DeleteAccountError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<DeleteAccountError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::ServerUnreachable => UiError(DeleteAccountError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(DeleteAccountError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminDisappearAccount {
    InsufficientPermission,
    UsernameNotFound,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminDisappearAccount> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminDisappearAccount::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminDisappearAccount::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminDisappearAccount::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminDisappearAccount::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminDisappearFileError {
    InsufficientPermission,
    FileNotFound,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminDisappearFileError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminDisappearFileError::InsufficientPermission)
            }
            CoreError::FileNonexistent => UiError(AdminDisappearFileError::FileNotFound),
            CoreError::ServerUnreachable => UiError(AdminDisappearFileError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(AdminDisappearFileError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminServerValidateError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
    UserNotFound,
}

impl From<CoreError> for Error<AdminServerValidateError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminServerValidateError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminServerValidateError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(AdminServerValidateError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminListUsersError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminListUsersError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminListUsersError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminListUsersError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminListUsersError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminRebuildIndexError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminRebuildIndexError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminRebuildIndexError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminRebuildIndexError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(AdminRebuildIndexError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminGetAccountInfoError {
    InsufficientPermission,
    UsernameNotFound,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminGetAccountInfoError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminGetAccountInfoError::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminGetAccountInfoError::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminGetAccountInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
                UiError(AdminGetAccountInfoError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminFileInfoError {
    InsufficientPermission,
    FileNotFound,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminFileInfoError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminFileInfoError::InsufficientPermission)
            }
            CoreError::FileNonexistent => UiError(AdminFileInfoError::FileNotFound),
            CoreError::ServerUnreachable => UiError(AdminFileInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminFileInfoError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FeatureFlagError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<FeatureFlagError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => UiError(FeatureFlagError::InsufficientPermission),
            CoreError::ServerUnreachable => UiError(FeatureFlagError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(FeatureFlagError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", e),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminSetUserTierError {
    InsufficientPermission,
    UsernameNotFound,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<CoreError> for Error<AdminSetUserTierError> {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::InsufficientPermission => {
                UiError(AdminSetUserTierError::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminSetUserTierError::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminSetUserTierError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminSetUserTierError::ClientUpdateRequired),
            CoreError::ExistingRequestPending => {
                UiError(AdminSetUserTierError::ExistingRequestPending)
            }
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
    CycleDetected(HashSet<Uuid>),
    FileNameEmpty(Uuid),
    FileNameTooLong(Uuid),
    FileNameContainsSlash(Uuid),
    PathConflict(HashSet<Uuid>),
    NonDecryptableFileName(Uuid),
    FileWithDifferentOwnerParent(Uuid),
    SharedLink { link: Uuid, shared_ancestor: Uuid },
    DuplicateLink { target: Uuid },
    BrokenLink(Uuid),
    OwnedLink(Uuid),
    DocumentReadError(Uuid, CoreError),
    Core(CoreError),
    Shared(SharedError),
}

impl From<SharedError> for TestRepoError {
    fn from(err: SharedError) -> Self {
        match err {
            SharedError::ValidationFailure(validation) => match validation {
                ValidationFailure::Orphan(id) => Self::FileOrphaned(id),
                ValidationFailure::Cycle(ids) => Self::CycleDetected(ids),
                ValidationFailure::PathConflict(ids) => Self::PathConflict(ids),
                ValidationFailure::NonFolderWithChildren(id) => Self::DocumentTreatedAsFolder(id),
                ValidationFailure::NonDecryptableFileName(id) => Self::NonDecryptableFileName(id),
                ValidationFailure::SharedLink { link, shared_ancestor } => {
                    Self::SharedLink { link, shared_ancestor }
                }
                ValidationFailure::DuplicateLink { target } => Self::DuplicateLink { target },
                ValidationFailure::BrokenLink(id) => Self::BrokenLink(id),
                ValidationFailure::OwnedLink(id) => Self::OwnedLink(id),
                ValidationFailure::FileWithDifferentOwnerParent(id) => {
                    Self::FileWithDifferentOwnerParent(id)
                }
                ValidationFailure::FileNameTooLong(id) => Self::FileNameTooLong(id),
            },
            _ => Self::Shared(err),
        }
    }
}

impl fmt::Display for TestRepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TestRepoError::*;
        match self {
            NoAccount => write!(f, "no account"),
            NoRootFolder => write!(f, "no root folder"),
            DocumentTreatedAsFolder(id) => write!(f, "doc '{}' treated as folder", id),
            FileOrphaned(id) => write!(f, "orphaned file '{}'", id),
            FileNameTooLong(id) => write!(f, "file '{}' is too long", id),
            CycleDetected(ids) => write!(f, "cycle for files: {}", ids.iter().join(", ")),
            FileNameEmpty(id) => write!(f, "file '{}' name is empty", id),
            FileNameContainsSlash(id) => write!(f, "file '{}' name contains slash", id),
            PathConflict(ids) => write!(f, "path conflict between: {}", ids.iter().join(", ")),
            NonDecryptableFileName(id) => write!(f, "can't decrypt file '{}' name", id),
            FileWithDifferentOwnerParent(id) => write!(f, "file '{}' different owner parent", id),
            SharedLink { link, shared_ancestor } => {
                write!(f, "shared link: {}, ancestor: {}", link, shared_ancestor)
            }
            DuplicateLink { target } => write!(f, "duplicate link '{}'", target),
            BrokenLink(id) => write!(f, "broken link '{}'", id),
            OwnedLink(id) => write!(f, "owned link '{}'", id),
            DocumentReadError(id, err) => write!(f, "doc '{}' read err: {:#?}", id, err),
            Core(err) => write!(f, "core err: {:#?}", err),
            Shared(err) => write!(f, "shared err: {:#?}", err),
        }
    }
}

impl From<CoreError> for TestRepoError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::AccountNonexistent => Self::NoAccount,
            _ => Self::Core(err),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Warning {
    EmptyFile(Uuid),
    InvalidUTF8(Uuid),
    UnreadableDrawing(Uuid),
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile(id) => write!(f, "empty file: {}", id),
            Self::InvalidUTF8(id) => write!(f, "invalid utf8 in file: {}", id),
            Self::UnreadableDrawing(id) => write!(f, "unreadable drawing: {}", id),
        }
    }
}
