use std::backtrace::Backtrace;
use std::collections::HashSet;
use std::fmt::Display;
use std::fmt::{self, Formatter};
use std::io;
use std::sync::PoisonError;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use crate::model::{SharedError, SharedErrorKind, ValidationFailure};
use crate::service::network::ApiError;

use super::api;

pub type LbResult<T> = Result<T, LbErr>;

#[derive(Debug)]
pub struct LbErr {
    pub kind: LbErrKind,
    pub backtrace: Option<Backtrace>,
}

/// Using this within core has limited meaning as the unexpected / expected error
/// calculation that happens in lib.rs won't have taken place. So in some sense
/// printing this out anywhere within core is going to be _unexpected_
impl Display for LbErr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Display for LbErrKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            LbErrKind::AccountExists => write!(f, "An account already exists"),
            LbErrKind::AccountNonexistent => write!(f, "You need an account to do that"),
            LbErrKind::AccountStringCorrupted => write!(f, "That account key is invalid"),
            LbErrKind::AlreadyCanceled => write!(f, "Your subscription has already been cancelled"),
            LbErrKind::AlreadyPremium => write!(f, "Your account is already premium"),
            LbErrKind::AppStoreAccountAlreadyLinked => {
                write!(f, "Your account is already linked to the App Store")
            }
            LbErrKind::CannotCancelSubscriptionForAppStore => {
                write!(f, "You cannot cancel an app store subscription from here")
            }
            LbErrKind::CardDecline => write!(f, "Your card was declined"),
            LbErrKind::CardExpired => write!(f, "Your card is expired"),
            LbErrKind::CardInsufficientFunds => write!(f, "This card has insufficient funds"),
            LbErrKind::CardInvalidCvc => write!(f, "Your CVC is invalid"),
            LbErrKind::CardInvalidExpMonth => write!(f, "Your expiration month is invalid"),
            LbErrKind::CardInvalidExpYear => write!(f, "Your expiration year is invalid"),
            LbErrKind::CardInvalidNumber => write!(f, "Your card number is invalid"),
            LbErrKind::CardNotSupported => write!(f, "That card is not supported by stripe"),
            LbErrKind::ClientUpdateRequired => {
                write!(f, "You must update your Lockbook to do that")
            }
            LbErrKind::CurrentUsageIsMoreThanNewTier => {
                write!(f, "You need to delete some files before downgrading your usage")
            }
            LbErrKind::DiskPathInvalid => write!(f, "That disk path is invalid"),
            LbErrKind::DiskPathTaken => write!(f, "That disk path is not available"),
            LbErrKind::DrawingInvalid => write!(f, "That drawing is invalid"),
            LbErrKind::ExistingRequestPending => {
                write!(f, "Existing billing request in progress, please wait and try again")
            }
            LbErrKind::FileNameContainsSlash => write!(f, "A file name cannot contain slashes"),
            LbErrKind::FileNameTooLong => write!(f, "That file name is too long"),
            LbErrKind::FileNameEmpty => write!(f, "A file name cannot be empty"),
            LbErrKind::FileNonexistent => write!(f, "That file does not exist"),
            LbErrKind::FileNotDocument => write!(f, "That file is not a document"),
            LbErrKind::FileNotFolder => write!(f, "That file is not a folder"),
            LbErrKind::FileParentNonexistent => write!(f, "Could not find that file parent"),
            LbErrKind::FolderMovedIntoSelf => write!(f, "You cannot move a folder into itself"),
            LbErrKind::InsufficientPermission => {
                write!(f, "You don't have the permission to do that")
            }
            LbErrKind::InvalidPurchaseToken => write!(f, "That purchase token is invalid"),
            LbErrKind::InvalidAuthDetails => {
                write!(f, "Our server failed to authenticate your request, please try again")
            }
            LbErrKind::KeyPhraseInvalid => {
                write!(f, "Your private key phrase is wrong")
            }
            LbErrKind::LinkInSharedFolder => {
                write!(f, "You cannot move a link into a shared folder")
            }
            LbErrKind::LinkTargetIsOwned => {
                write!(f, "You cannot create a link to a file that you own")
            }
            LbErrKind::LinkTargetNonexistent => write!(f, "That link target does not exist"),
            LbErrKind::MultipleLinksToSameFile => {
                write!(f, "You cannot have multiple links to the same file")
            }
            LbErrKind::NotPremium => write!(f, "You do not have a premium subscription"),
            LbErrKind::UsageIsOverDataCap => {
                write!(f, "You're out of space")
            }
            LbErrKind::UsageIsOverFreeTierDataCap => {
                write!(f, "You're out of space, you can purchase additional space")
            }
            LbErrKind::OldCardDoesNotExist => write!(f, "No existing card found"),
            LbErrKind::PathContainsEmptyFileName => {
                write!(f, "That path contains an empty file name")
            }
            LbErrKind::PathTaken => write!(f, "That path is not available"),
            LbErrKind::RootModificationInvalid => write!(f, "You cannot modify your root"),
            LbErrKind::RootNonexistent => write!(f, "Could not find your root file"),
            LbErrKind::ServerDisabled => write!(
                f,
                "The server is not accepting this action at the moment, please try again later"
            ),
            LbErrKind::ServerUnreachable => write!(f, "Could not reach server"),
            LbErrKind::ShareAlreadyExists => write!(f, "That share already exists"),
            LbErrKind::ShareNonexistent => write!(f, "That share does not exist"),
            LbErrKind::TryAgain => write!(f, "Please try again"),
            LbErrKind::UsernameInvalid => write!(f, "That username is invalid"),
            LbErrKind::UsernameNotFound => write!(f, "That username is not found"),
            LbErrKind::UsernamePublicKeyMismatch => {
                write!(f, "That username doesn't match that public key")
            }
            LbErrKind::UsernameTaken => write!(f, "That username is not available"),
            LbErrKind::Unexpected(msg) => write!(f, "Unexpected error: {msg}"),
            LbErrKind::AlreadySyncing => {
                write!(f, "A sync is already in progress, cannot begin another sync at this time!")
            }
            LbErrKind::ReReadRequired => {
                write!(f, "This document changed since you last read it, please re-read it!")
            }
            LbErrKind::Validation(validation_failure) => todo!(),
            LbErrKind::Diff(diff_error) => todo!(),
            LbErrKind::Sign(sign_error) => todo!(),
        }
    }
}

impl From<LbErrKind> for LbErr {
    fn from(kind: LbErrKind) -> Self {
        Self { kind, backtrace: Some(Backtrace::force_capture()) }
    }
}

impl From<SharedError> for LbErr {
    fn from(err: SharedError) -> Self {
        let kind = match err.kind {
            _ => LbErrKind::Unexpected(format!("unexpected shared error {:?}", err)),
        };
        Self { kind, backtrace: err.backtrace }
    }
}

#[derive(Debug)]
pub struct UnexpectedError {
    pub msg: String,
    pub backtrace: Option<Backtrace>,
}

impl UnexpectedError {
    pub fn new(s: impl ToString) -> Self {
        Self { msg: s.to_string(), backtrace: Some(Backtrace::force_capture()) }
    }
}

impl fmt::Display for UnexpectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected error: {}", self.msg)
    }
}

impl From<LbErr> for UnexpectedError {
    fn from(err: LbErr) -> Self {
        Self { msg: format!("{:?}", err.kind), backtrace: err.backtrace }
    }
}

impl<T> From<PoisonError<T>> for UnexpectedError {
    fn from(err: PoisonError<T>) -> Self {
        Self::new(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvError) -> Self {
        Self::new(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvTimeoutError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvTimeoutError) -> Self {
        Self::new(format!("{:#?}", err))
    }
}

impl<T> From<crossbeam::channel::SendError<T>> for UnexpectedError {
    fn from(err: crossbeam::channel::SendError<T>) -> Self {
        Self::new(format!("{:#?}", err))
    }
}

impl Serialize for UnexpectedError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UnexpectedError", 2)?;
        state.serialize_field("tag", "Unexpected")?;
        state.serialize_field("content", &self.msg)?;
        state.end()
    }
}

#[macro_export]
macro_rules! unexpected_only {
    ($base:literal $(, $args:tt )*) => {{
        debug!($base $(, $args )*);
        debug!("{:?}", std::backtrace::Backtrace::force_capture());
        debug!($base $(, $args )*);
        UnexpectedError::new(format!($base $(, $args )*))
    }};
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LbErrKind {
    AccountExists,
    AccountNonexistent,
    AccountStringCorrupted,
    AlreadyCanceled,
    AlreadyPremium,
    AppStoreAccountAlreadyLinked,
    AlreadySyncing,
    CannotCancelSubscriptionForAppStore,
    CardDecline,
    CardExpired,
    CardInsufficientFunds,
    CardInvalidCvc,
    CardInvalidExpMonth,
    CardInvalidExpYear,
    CardInvalidNumber,
    CardNotSupported,
    ClientUpdateRequired,
    CurrentUsageIsMoreThanNewTier,
    DiskPathInvalid,
    DiskPathTaken,
    DrawingInvalid,
    ExistingRequestPending,
    FileNameContainsSlash,
    FileNameTooLong,
    FileNameEmpty,
    FileNonexistent,
    FileNotDocument,
    FileNotFolder,
    FileParentNonexistent,
    FolderMovedIntoSelf,
    InsufficientPermission,
    InvalidPurchaseToken,
    InvalidAuthDetails,
    KeyPhraseInvalid,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    MultipleLinksToSameFile,
    NotPremium,
    UsageIsOverDataCap,
    UsageIsOverFreeTierDataCap,
    OldCardDoesNotExist,
    PathContainsEmptyFileName,
    PathTaken,
    RootModificationInvalid,
    RootNonexistent,
    ServerDisabled,
    ServerUnreachable,
    ShareAlreadyExists,
    ShareNonexistent,
    TryAgain,
    UsernameInvalid,
    UsernameNotFound,
    UsernamePublicKeyMismatch,
    UsernameTaken,
    ReReadRequired,
    Diff(DiffError),
    Validation(ValidationFailure),
    Sign(SignError),

    /// If no programmer in any part of the stack (including tests) expects
    /// to see a particular error, we debug format the underlying error to
    /// keep the number of error types in check. Commonly used for errors
    /// originating in other crates.
    Unexpected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    OldVersionIncorrect,
    OldFileNotFound,
    OldVersionRequired,
    DiffMalformed,
    HmacModificationInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    SignatureInvalid,
    SignatureParseError(libsecp256k1::Error),
    WrongPublicKey,
    SignatureInTheFuture(u64),
    SignatureExpired(u64),
}

impl From<bincode::Error> for LbErr {
    fn from(err: bincode::Error) -> Self {
        core_err_unexpected(err).into()
    }
}

pub fn core_err_unexpected<T: fmt::Debug>(err: T) -> LbErrKind {
    LbErrKind::Unexpected(format!("{:?}", err))
}

// todo call location becomes useless here, and we want that
pub fn unexpected<T: fmt::Debug>(err: T) -> LbErr {
    LbErrKind::Unexpected(format!("{:?}", err)).into()
}

pub trait Unexpected {
    fn unexpected_err(self) -> LbErr;
}

impl From<db_rs::DbError> for LbErr {
    fn from(err: db_rs::DbError) -> Self {
        core_err_unexpected(err).into()
    }
}

impl<G> From<PoisonError<G>> for LbErr {
    fn from(err: PoisonError<G>) -> Self {
        core_err_unexpected(err).into()
    }
}

impl From<io::Error> for LbErr {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound
            | io::ErrorKind::PermissionDenied
            | io::ErrorKind::InvalidInput => LbErrKind::DiskPathInvalid,
            io::ErrorKind::AlreadyExists => LbErrKind::DiskPathTaken,
            _ => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<serde_json::Error> for LbErr {
    fn from(err: serde_json::Error) -> Self {
        LbErrKind::Unexpected(format!("{err}")).into()
    }
}

impl From<ApiError<api::NewAccountError>> for LbErr {
    fn from(err: ApiError<api::NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            ApiError::Endpoint(api::NewAccountError::UsernameTaken) => LbErrKind::UsernameTaken,
            ApiError::Endpoint(api::NewAccountError::InvalidUsername) => LbErrKind::UsernameInvalid,
            ApiError::Endpoint(api::NewAccountError::Disabled) => LbErrKind::ServerDisabled,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetPublicKeyError>> for LbErr {
    fn from(err: ApiError<api::GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            ApiError::Endpoint(api::GetPublicKeyError::UserNotFound) => {
                LbErrKind::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsernameError>> for LbErr {
    fn from(err: ApiError<api::GetUsernameError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            ApiError::Endpoint(api::GetUsernameError::UserNotFound) => {
                LbErrKind::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetFileIdsError>> for LbErr {
    fn from(e: ApiError<api::GetFileIdsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUpdatesError>> for LbErr {
    fn from(e: ApiError<api::GetUpdatesError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetDocumentError>> for LbErr {
    fn from(e: ApiError<api::GetDocumentError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::UpsertError>> for LbErr {
    fn from(e: ApiError<api::UpsertError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::Endpoint(api::UpsertError::UsageIsOverDataCap) => {
                LbErrKind::UsageIsOverDataCap
            }
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::ChangeDocError>> for LbErr {
    fn from(e: ApiError<api::ChangeDocError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::Endpoint(api::ChangeDocError::UsageIsOverDataCap) => {
                LbErrKind::UsageIsOverDataCap
            }
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsageError>> for LbErr {
    fn from(e: ApiError<api::GetUsageError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Warning {
    EmptyFile(Uuid),
    InvalidUTF8(Uuid),
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile(id) => write!(f, "empty file: {}", id),
            Self::InvalidUTF8(id) => write!(f, "invalid utf8 in file: {}", id),
        }
    }
}
