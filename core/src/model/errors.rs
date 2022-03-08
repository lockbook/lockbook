use std::fmt::{Display, Formatter};
use std::io::ErrorKind;

use lockbook_models::tree::TreeError;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::service::api_service::ApiError;

#[derive(Debug)]
pub struct UnexpectedError(pub String);

impl Display for UnexpectedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected error: {}", self.0)
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
