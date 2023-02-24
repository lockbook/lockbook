use std::backtrace::Backtrace;
use std::io;

use db_rs::DbError;

use crate::LbErrorKind;
use crate::ValidationFailure;

pub type LbResult<T> = Result<T, LbError>;

#[derive(Debug)]
pub struct LbError {
    pub kind: LbErrorKind,
    pub backtrace: Option<Backtrace>,
}

impl From<LbErrorKind> for LbError {
    fn from(kind: LbErrorKind) -> Self {
        let kind = match kind {
            LbErrorKind::DeletedFileUpdated(_) => LbErrorKind::FileNonexistent,
            LbErrorKind::DuplicateShare => LbErrorKind::ShareAlreadyExists,
            LbErrorKind::ValidationFailure(ref vf) => match vf {
                ValidationFailure::Orphan(_id) => kind,
                ValidationFailure::Cycle(_) => LbErrorKind::FolderMovedIntoSelf,
                ValidationFailure::PathConflict(_) => LbErrorKind::PathTaken,
                ValidationFailure::SharedLink { .. } => LbErrorKind::LinkInSharedFolder,
                ValidationFailure::DuplicateLink { .. } => LbErrorKind::MultipleLinksToSameFile,
                ValidationFailure::BrokenLink(_) => LbErrorKind::LinkTargetNonexistent,
                ValidationFailure::OwnedLink(_) => LbErrorKind::LinkTargetIsOwned,
                ValidationFailure::NonFolderWithChildren(_) => LbErrorKind::FileNotFolder,
                vf => LbErrorKind::Unexpected(format!("unexpected validation failure {:?}", vf)),
            },
            _ => kind,
        };
        Self { kind, backtrace: Some(Backtrace::capture()) }
    }
}

// impl fmt::Display for LbError

impl LbError {
    pub fn code() -> i32 {
        todo!()
    }
}

impl From<DbError> for LbError {
    fn from(value: DbError) -> Self {
        LbErrorKind::Db(format!("db error: {:?}", value)).into()
    }
}

impl From<hmdb::errors::Error> for LbError {
    fn from(err: hmdb::errors::Error) -> Self {
        LbErrorKind::Unexpected(format!("{:?}", err)).into()
    }
}

impl From<bincode::Error> for LbError {
    fn from(err: bincode::Error) -> Self {
        LbErrorKind::BincodeError(err.to_string()).into()
    }
}

impl From<io::Error> for LbError {
    fn from(err: io::Error) -> Self {
        LbErrorKind::Io(err.to_string()).into()
    }
}

impl<G> From<std::sync::PoisonError<G>> for LbError {
    fn from(err: std::sync::PoisonError<G>) -> Self {
        LbErrorKind::Unexpected(format!("{:?}", err)).into()
    }
}

impl From<serde_json::Error> for LbError {
    fn from(err: serde_json::Error) -> Self {
        LbErrorKind::Unexpected(format!("{err}")).into()
    }
}

/*
pub fn lb_err_unexpected<T: fmt::Debug>(err: T) -> LbErrorKind {
    LbErrorKind::Unexpected(format!("{:#?}", err))
}

impl From<ApiError<api::NewAccountError>> for LbError {
    fn from(err: ApiError<api::NewAccountError>) -> Self {
        match err {
            ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
            ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
            ApiError::Endpoint(api::NewAccountError::UsernameTaken) => LbErrorKind::UsernameTaken,
            ApiError::Endpoint(api::NewAccountError::InvalidUsername) => LbErrorKind::UsernameInvalid,
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
*/
