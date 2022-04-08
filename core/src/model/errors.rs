use std::fmt::{Display, Formatter};
use std::io::ErrorKind;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use strum_macros::EnumIter;

use lockbook_models::api::{GetPublicKeyError, NewAccountError};
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
    NewTierIsOldTier,
    NotAStripeCustomer,
    PathContainsEmptyFileName,
    PathNonexistent,
    PathStartsWithNonRoot,
    PathTaken,
    OldCardDoesNotExist,
    RootModificationInvalid,
    RootNonexistent,
    ServerUnreachable,
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

impl<T: std::fmt::Debug> From<ApiError<T>> for CoreError {
    fn from(e: ApiError<T>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
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

impl From<ApiError<NewAccountError>> for Error<CreateAccountError> {
    fn from(err: ApiError<NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable.into(),
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired.into(),
            ApiError::Endpoint(NewAccountError::UsernameTaken) => CoreError::UsernameTaken.into(),
            ApiError::Endpoint(NewAccountError::InvalidUsername) => {
                CoreError::UsernameInvalid.into()
            }
            ApiError::Endpoint(NewAccountError::Disabled) => CoreError::ServerDisabled.into(),
            e => core_err_unexpected(e).into(),
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

impl From<ApiError<GetPublicKeyError>> for Error<ImportError> {
    fn from(err: ApiError<GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable.into(),
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired.into(),
            ApiError::Endpoint(GetPublicKeyError::UserNotFound) => {
                CoreError::AccountNonexistent.into()
            }
            e => core_err_unexpected(e).into(),
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
