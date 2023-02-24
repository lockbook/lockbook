use std::collections::HashSet;
use std::fmt;
use std::io;
use std::sync::PoisonError;

use itertools::Itertools;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use uuid::Uuid;

use lockbook_shared::{api, ValidationFailure};
use lockbook_shared::{LbError, LbErrorKind};

use crate::service::api_service::ApiError;

#[derive(Debug)]
pub struct UnexpectedError(pub String);

impl fmt::Display for UnexpectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected error: {}", self.0)
    }
}

/*impl From<CoreError> for UnexpectedError {
    fn from(e: CoreError) -> Self {
        Self(format!("{:?}", e))
    }
}*/

impl<T> From<std::sync::PoisonError<T>> for UnexpectedError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Self(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvError) -> Self {
        Self(format!("{:#?}", err))
    }
}

impl From<crossbeam::channel::RecvTimeoutError> for UnexpectedError {
    fn from(err: crossbeam::channel::RecvTimeoutError) -> Self {
        Self(format!("{:#?}", err))
    }
}

impl<T> From<crossbeam::channel::SendError<T>> for UnexpectedError {
    fn from(err: crossbeam::channel::SendError<T>) -> Self {
        Self(format!("{:#?}", err))
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

// FROM API IMPLS

pub fn lb_err_unexpected<T: fmt::Debug>(err: T) -> LbErrorKind {
    LbErrorKind::Unexpected(format!("{:#?}", err))
}

impl From<ApiError<api::NewAccountError>> for LbError {
    fn from(err: ApiError<api::NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            ApiError::Endpoint(api::NewAccountError::UsernameTaken) => LbErrorKind::UsernameTaken,
            ApiError::Endpoint(api::NewAccountError::InvalidUsername) => {
                LbErrorKind::UsernameInvalid
            }
            ApiError::Endpoint(api::NewAccountError::Disabled) => LbErrorKind::ServerDisabled,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetPublicKeyError>> for LbError {
    fn from(err: ApiError<api::GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            ApiError::Endpoint(api::GetPublicKeyError::UserNotFound) => {
                LbErrorKind::AccountNonexistent
            }
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsernameError>> for LbError {
    fn from(err: ApiError<api::GetUsernameError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            ApiError::Endpoint(api::GetUsernameError::UserNotFound) => {
                LbErrorKind::AccountNonexistent
            }
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetFileIdsError>> for LbError {
    fn from(e: ApiError<api::GetFileIdsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUpdatesError>> for LbError {
    fn from(e: ApiError<api::GetUpdatesError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetDocumentError>> for LbError {
    fn from(e: ApiError<api::GetDocumentError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::UpsertError>> for LbError {
    fn from(e: ApiError<api::UpsertError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::ChangeDocError>> for LbError {
    fn from(e: ApiError<api::ChangeDocError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
}

impl From<ApiError<api::GetUsageError>> for LbError {
    fn from(e: ApiError<api::GetUsageError>) -> Self {
        match e {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            e => lb_err_unexpected(e),
        }
        .into()
    }
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

/*impl From<LbError> for CoreError {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::RootNonexistent => Self::RootNonexistent,
            LbErrorKind::FileNonexistent => Self::FileNonexistent,
            LbErrorKind::FileParentNonexistent => Self::FileParentNonexistent,
            LbErrorKind::Unexpected(err) => Self::Unexpected(err),
            LbErrorKind::PathContainsEmptyFileName => Self::PathContainsEmptyFileName,
            LbErrorKind::PathTaken => Self::PathTaken,
            LbErrorKind::FileNameContainsSlash => Self::FileNameContainsSlash,
            LbErrorKind::RootModificationInvalid => Self::RootModificationInvalid,
            LbErrorKind::DeletedFileUpdated(_) => Self::FileNonexistent,
            LbErrorKind::FileNameEmpty => Self::FileNameEmpty,
            LbErrorKind::FileNotFolder => Self::FileNotFolder,
            LbErrorKind::FileNotDocument => Self::FileNotDocument,
            LbErrorKind::InsufficientPermission => Self::InsufficientPermission,
            LbErrorKind::ShareNonexistent => Self::ShareNonexistent,
            LbErrorKind::DuplicateShare => Self::ShareAlreadyExists,
            LbErrorKind::ValidationFailure(failure) => match failure {
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
}*/

/*impl From<db_rs::DbError> for CoreError {
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

impl From<ApiError<api::NewAccountError>> for CoreError {
    fn from(err: ApiError<api::NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::NewAccountError::UsernameTaken) => CoreError::UsernameTaken,
            ApiError::Endpoint(api::NewAccountError::InvalidUsername) => CoreError::UsernameInvalid,
            ApiError::Endpoint(api::NewAccountError::Disabled) => CoreError::ServerDisabled,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::GetPublicKeyError>> for CoreError {
    fn from(err: ApiError<api::GetPublicKeyError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::GetPublicKeyError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::GetUsernameError>> for CoreError {
    fn from(err: ApiError<api::GetUsernameError>) -> Self {
        match err {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            ApiError::Endpoint(api::GetUsernameError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::GetFileIdsError>> for CoreError {
    fn from(e: ApiError<api::GetFileIdsError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}

impl From<ApiError<api::GetUpdatesError>> for CoreError {
    fn from(e: ApiError<api::GetUpdatesError>) -> Self {
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

impl From<ApiError<api::GetUsageError>> for CoreError {
    fn from(e: ApiError<api::GetUsageError>) -> Self {
        match e {
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            e => core_err_unexpected(e),
        }
    }
}*/

impl From<LbError> for UnexpectedError {
    fn from(err: LbError) -> Self {
        Self(format!("{:?}", err))
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
    FileNameContainsSlash(Uuid),
    PathConflict(HashSet<Uuid>),
    NonDecryptableFileName(Uuid),
    FileWithDifferentOwnerParent(Uuid),
    SharedLink { link: Uuid, shared_ancestor: Uuid },
    DuplicateLink { target: Uuid },
    BrokenLink(Uuid),
    OwnedLink(Uuid),
    DocumentReadError(Uuid, LbError),
    Other(LbError),
}

impl From<LbError> for TestRepoError {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::ValidationFailure(vf) => match vf {
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
            },
            _ => Self::Other(err),
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
            Other(err) => write!(f, "error: {:#?}", err),
        }
    }
}

/*impl From<CoreError> for TestRepoError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::AccountNonexistent => Self::NoAccount,
            _ => Self::Core(err),
        }
    }
}*/

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
