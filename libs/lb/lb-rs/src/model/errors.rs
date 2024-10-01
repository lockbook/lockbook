use std::backtrace::Backtrace;
use std::collections::HashSet;
use std::fmt::Display;
use std::fmt::{self, Formatter};
use std::io;
use std::sync::PoisonError;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use crate::shared::{api, SharedError, SharedErrorKind, ValidationFailure};

use crate::service::api_service::ApiError;

pub type LbResult<T> = Result<T, LbError>;

#[derive(Debug)]
pub struct LbError {
    pub kind: CoreError,
    pub backtrace: Option<Backtrace>,
}

/// Using this within core has limited meaning as the unexpected / expected error
/// calculation that happens in lib.rs won't have taken place. So in some sense
/// printing this out anywhere within core is going to be _unexpected_
impl Display for LbError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.backtrace {
            Some(backtrace) => {
                writeln!(f, "unexpected error: {:?}: {}", self.kind, self.kind).unwrap();
                writeln!(f, "{backtrace}").unwrap();
                Ok(())
            }
            None => write!(f, "{}", self.kind),
        }
    }
}

impl Display for CoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::AccountExists => write!(f, "an account already exists"),
            CoreError::AccountNonexistent => write!(f, "you need an account to do that"),
            CoreError::AccountStringCorrupted => write!(f, "Account String corrupted"),
            CoreError::AlreadyCanceled => write!(f, "your subscription has already been cancelled"),
            CoreError::AlreadyPremium => write!(f, "your account is already premium"),
            CoreError::AppStoreAccountAlreadyLinked => {
                write!(f, "your account is already linked to the App Store")
            }
            CoreError::CannotCancelSubscriptionForAppStore => {
                write!(f, "you cannot cancel an app store subscription from here")
            }
            CoreError::CardDecline => write!(f, "your card was declined"),
            CoreError::CardExpired => write!(f, "your card is expired"),
            CoreError::CardInsufficientFunds => write!(f, "this card has insufficient funds"),
            CoreError::CardInvalidCvc => write!(f, "invalid cvc"),
            CoreError::CardInvalidExpMonth => write!(f, "invalid expiration month"),
            CoreError::CardInvalidExpYear => write!(f, "invalid expiration year"),
            CoreError::CardInvalidNumber => write!(f, "invalid card number"),
            CoreError::CardNotSupported => write!(f, "card not supported by stripe"),
            CoreError::ClientUpdateRequired => {
                write!(f, "you need a newer version of lockbook to do that")
            }
            CoreError::CurrentUsageIsMoreThanNewTier => {
                write!(f, "you need to delete some files before downgrading your usage")
            }
            CoreError::DiskPathInvalid => write!(f, "disk path invalid"),
            CoreError::DiskPathTaken => write!(f, "disk path not available"),
            CoreError::DrawingInvalid => write!(f, "not a valid drawing"),
            CoreError::ExistingRequestPending => {
                write!(f, "existing billing request in progress, please wait and try again")
            }
            CoreError::FileNameContainsSlash => write!(f, "file names cannot contain slashes"),
            CoreError::FileNameTooLong => write!(f, "that file name is too long"),
            CoreError::FileNameEmpty => write!(f, "file name cannot be empty"),
            CoreError::FileNonexistent => write!(f, "that file does not exist"),
            CoreError::FileNotDocument => write!(f, "that file is not a document"),
            CoreError::FileNotFolder => write!(f, "that file is not a folder"),
            CoreError::FileParentNonexistent => write!(f, "could not find a parent"),
            CoreError::FolderMovedIntoSelf => write!(f, "you cannot move a folder into itself"),
            CoreError::InsufficientPermission => {
                write!(f, "you don't have the permission to do that")
            }
            CoreError::InvalidPurchaseToken => write!(f, "invalid purchase token"),
            CoreError::InvalidAuthDetails => {
                write!(f, "our server failed to authenticate your request, please try again")
            }
            CoreError::KeyPhraseInvalid => {
                write!(f, "your private key phrase is wrong")
            }
            CoreError::LinkInSharedFolder => {
                write!(f, "you cannot move a link into a shared folder")
            }
            CoreError::LinkTargetIsOwned => {
                write!(f, "you cannot create a link to a file that you own")
            }
            CoreError::LinkTargetNonexistent => write!(f, "that link target does not exist"),
            CoreError::MultipleLinksToSameFile => {
                write!(f, "you cannot have multiple links to the same file")
            }
            CoreError::NotPremium => write!(f, "you do not currently have a premium subscription"),
            CoreError::UsageIsOverDataCap => {
                write!(f, "you're out of space")
            }
            CoreError::UsageIsOverFreeTierDataCap => {
                write!(f, "you're out of space, you can purchase additional space")
            }
            CoreError::OldCardDoesNotExist => write!(f, "no existing card found"),
            CoreError::PathContainsEmptyFileName => {
                write!(f, "that path contains an empty file name")
            }
            CoreError::PathTaken => write!(f, "that path is not available"),
            CoreError::RootModificationInvalid => write!(f, "you cannot modify your root"),
            CoreError::RootNonexistent => write!(f, "no root found"),
            CoreError::ServerDisabled => write!(
                f,
                "the server is not accepting this action at the moment, please try again later"
            ),
            CoreError::ServerUnreachable => write!(f, "could not reach server"),
            CoreError::ShareAlreadyExists => write!(f, "that share already exists"),
            CoreError::ShareNonexistent => write!(f, "share non-existent"),
            CoreError::TryAgain => write!(f, "please try again"),
            CoreError::UsernameInvalid => write!(f, "that username is invalid"),
            CoreError::UsernameNotFound => write!(f, "username not found"),
            CoreError::UsernamePublicKeyMismatch => {
                write!(f, "that username doesn't match that public key")
            }
            CoreError::UsernameTaken => write!(f, "username not available"),
            CoreError::Unexpected(msg) => write!(f, "unexpected error: {msg}"),
            CoreError::AlreadySyncing => {
                write!(f, "A sync is already in progress, cannot begin another sync at this time!")
            }
            CoreError::ReReadRequired => {
                write!(f, "This document changed since you last read it, please re-read it!")
            }
        }
    }
}

impl From<CoreError> for LbError {
    fn from(kind: CoreError) -> Self {
        Self { kind, backtrace: Some(Backtrace::force_capture()) }
    }
}

impl From<SharedError> for LbError {
    fn from(err: SharedError) -> Self {
        let kind = match err.kind {
            SharedErrorKind::RootNonexistent => CoreError::RootNonexistent,
            SharedErrorKind::FileNonexistent => CoreError::FileNonexistent,
            SharedErrorKind::FileParentNonexistent => CoreError::FileParentNonexistent,
            SharedErrorKind::Unexpected(err) => CoreError::Unexpected(err.to_string()),
            SharedErrorKind::PathContainsEmptyFileName => CoreError::PathContainsEmptyFileName,
            SharedErrorKind::PathTaken => CoreError::PathTaken,
            SharedErrorKind::FileNameContainsSlash => CoreError::FileNameContainsSlash,
            SharedErrorKind::RootModificationInvalid => CoreError::RootModificationInvalid,
            SharedErrorKind::DeletedFileUpdated(_) => CoreError::FileNonexistent,
            SharedErrorKind::FileNameEmpty => CoreError::FileNameEmpty,
            SharedErrorKind::FileNotFolder => CoreError::FileNotFolder,
            SharedErrorKind::FileNotDocument => CoreError::FileNotDocument,
            SharedErrorKind::InsufficientPermission => CoreError::InsufficientPermission,
            SharedErrorKind::ShareNonexistent => CoreError::ShareNonexistent,
            SharedErrorKind::DuplicateShare => CoreError::ShareAlreadyExists,
            SharedErrorKind::KeyPhraseInvalid => CoreError::KeyPhraseInvalid,
            SharedErrorKind::ValidationFailure(failure) => match failure {
                ValidationFailure::Cycle(_) => CoreError::FolderMovedIntoSelf,
                ValidationFailure::PathConflict(_) => CoreError::PathTaken,
                ValidationFailure::SharedLink { .. } => CoreError::LinkInSharedFolder,
                ValidationFailure::DuplicateLink { .. } => CoreError::MultipleLinksToSameFile,
                ValidationFailure::BrokenLink(_) => CoreError::LinkTargetNonexistent,
                ValidationFailure::OwnedLink(_) => CoreError::LinkTargetIsOwned,
                ValidationFailure::NonFolderWithChildren(_) => CoreError::FileNotFolder,
                vf => CoreError::Unexpected(format!("unexpected validation failure {:?}", vf)),
            },
            _ => CoreError::Unexpected(format!("unexpected shared error {:?}", err)),
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

impl From<LbError> for UnexpectedError {
    fn from(err: LbError) -> Self {
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
pub enum CoreError {
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
    Unexpected(String),
}

pub fn core_err_unexpected<T: fmt::Debug>(err: T) -> CoreError {
    CoreError::Unexpected(format!("{:#?}", err))
}

impl From<db_rs::DbError> for LbError {
    fn from(err: db_rs::DbError) -> Self {
        core_err_unexpected(err).into()
    }
}

impl<G> From<PoisonError<G>> for LbError {
    fn from(err: PoisonError<G>) -> Self {
        core_err_unexpected(err).into()
    }
}

impl From<io::Error> for LbError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound
            | io::ErrorKind::PermissionDenied
            | io::ErrorKind::InvalidInput => CoreError::DiskPathInvalid,
            io::ErrorKind::AlreadyExists => CoreError::DiskPathTaken,
            _ => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<serde_json::Error> for LbError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::Unexpected(format!("{err}")).into()
    }
}

impl From<ApiError<api::NewAccountError>> for LbError {
    fn from(err: ApiError<api::NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::NewAccountError::UsernameTaken) => CoreError::UsernameTaken,
            ApiError::Endpoint(api::NewAccountError::InvalidUsername) => CoreError::UsernameInvalid,
            ApiError::Endpoint(api::NewAccountError::Disabled) => CoreError::ServerDisabled,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetPublicKeyError>> for LbError {
    fn from(err: ApiError<api::GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::GetPublicKeyError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsernameError>> for LbError {
    fn from(err: ApiError<api::GetUsernameError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::GetUsernameError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetFileIdsError>> for LbError {
    fn from(e: ApiError<api::GetFileIdsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUpdatesError>> for LbError {
    fn from(e: ApiError<api::GetUpdatesError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetDocumentError>> for LbError {
    fn from(e: ApiError<api::GetDocumentError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::UpsertError>> for LbError {
    fn from(e: ApiError<api::UpsertError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::Endpoint(api::UpsertError::UsageIsOverDataCap) => {
                CoreError::UsageIsOverDataCap
            }
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::ChangeDocError>> for LbError {
    fn from(e: ApiError<api::ChangeDocError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::Endpoint(api::ChangeDocError::UsageIsOverDataCap) => {
                CoreError::UsageIsOverDataCap
            }
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsageError>> for LbError {
    fn from(e: ApiError<api::GetUsageError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
        .into()
    }
}

#[derive(Debug)]
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
    Core(LbError),
    Shared(SharedError),
}

impl From<SharedError> for TestRepoError {
    fn from(err: SharedError) -> Self {
        match err.kind {
            SharedErrorKind::ValidationFailure(validation) => match validation {
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
            CycleDetected(ids) => write!(f, "cycle for files: {:?}", ids),
            FileNameEmpty(id) => write!(f, "file '{}' name is empty", id),
            FileNameContainsSlash(id) => write!(f, "file '{}' name contains slash", id),
            FileNameTooLong(id) => write!(f, "file '{}' name is too long", id),
            PathConflict(ids) => write!(f, "path conflict between: {:?}", ids),
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

#[derive(Debug, Clone, Serialize)]
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
