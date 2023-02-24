use serde::Serialize;
use strum_macros::EnumIter;

use lockbook_core::{LbError, LbErrorKind};

use self::Error::UiError;

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

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateAccountError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
    ServerDisabled,
}

impl From<LbError> for Error<CreateAccountError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AccountExists => UiError(CreateAccountError::AccountExistsAlready),
            LbErrorKind::UsernameTaken => UiError(CreateAccountError::UsernameTaken),
            LbErrorKind::UsernameInvalid => UiError(CreateAccountError::InvalidUsername),
            LbErrorKind::ServerUnreachable => UiError(CreateAccountError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(CreateAccountError::ClientUpdateRequired),
            LbErrorKind::ServerDisabled => UiError(CreateAccountError::ServerDisabled),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<ImportError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AccountStringCorrupted => UiError(ImportError::AccountStringCorrupted),
            LbErrorKind::AccountExists => UiError(ImportError::AccountExistsAlready),
            LbErrorKind::UsernamePublicKeyMismatch => UiError(ImportError::UsernamePKMismatch),
            LbErrorKind::ServerUnreachable => UiError(ImportError::CouldNotReachServer),
            LbErrorKind::AccountNonexistent => UiError(ImportError::AccountDoesNotExist),
            LbErrorKind::ClientUpdateRequired => UiError(ImportError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AccountExportError {
    NoAccount,
}

impl From<LbError> for Error<AccountExportError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AccountNonexistent => UiError(AccountExportError::NoAccount),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAccountError {
    NoAccount,
}

impl From<LbError> for Error<GetAccountError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AccountNonexistent => UiError(GetAccountError::NoAccount),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<CreateFileAtPathError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::PathContainsEmptyFileName => {
                UiError(CreateFileAtPathError::PathContainsEmptyFile)
            }
            LbErrorKind::RootNonexistent => UiError(CreateFileAtPathError::NoRoot),
            LbErrorKind::PathTaken => UiError(CreateFileAtPathError::FileAlreadyExists),
            LbErrorKind::FileNotFolder => UiError(CreateFileAtPathError::DocumentTreatedAsFolder),
            LbErrorKind::InsufficientPermission => {
                UiError(CreateFileAtPathError::InsufficientPermission)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByPathError {
    NoFileAtThatPath,
}

impl From<LbError> for Error<GetFileByPathError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(GetFileByPathError::NoFileAtThatPath),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CreateFileError {
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameEmpty,
    FileNameContainsSlash,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    InsufficientPermission,
    MultipleLinksToSameFile,
}

impl From<LbError> for Error<CreateFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            LbErrorKind::PathTaken => UiError(CreateFileError::FileNameNotAvailable),
            LbErrorKind::FileNotFolder => UiError(CreateFileError::DocumentTreatedAsFolder),
            LbErrorKind::FileParentNonexistent => UiError(CreateFileError::CouldNotFindAParent),
            LbErrorKind::FileNameEmpty => UiError(CreateFileError::FileNameEmpty),
            LbErrorKind::FileNameContainsSlash => UiError(CreateFileError::FileNameContainsSlash),
            LbErrorKind::LinkInSharedFolder => UiError(CreateFileError::LinkInSharedFolder),
            LbErrorKind::LinkTargetIsOwned => UiError(CreateFileError::LinkTargetIsOwned),
            LbErrorKind::LinkTargetNonexistent => UiError(CreateFileError::LinkTargetNonexistent),
            LbErrorKind::InsufficientPermission => UiError(CreateFileError::InsufficientPermission),
            LbErrorKind::MultipleLinksToSameFile => {
                UiError(CreateFileError::MultipleLinksToSameFile)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum WriteToDocumentError {
    FileDoesNotExist,
    FolderTreatedAsDocument,
    InsufficientPermission,
}

impl From<LbError> for Error<WriteToDocumentError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(WriteToDocumentError::FileDoesNotExist),
            LbErrorKind::FileNotDocument => UiError(WriteToDocumentError::FolderTreatedAsDocument),
            LbErrorKind::InsufficientPermission => {
                UiError(WriteToDocumentError::InsufficientPermission)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetRootError {
    NoRoot,
}

impl From<LbError> for Error<GetRootError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::RootNonexistent => UiError(GetRootError::NoRoot),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetAndGetChildrenError {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
}

impl From<LbError> for Error<GetAndGetChildrenError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(GetAndGetChildrenError::FileDoesNotExist),
            LbErrorKind::FileNotFolder => UiError(GetAndGetChildrenError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetFileByIdError {
    NoFileWithThatId,
}

impl From<LbError> for Error<GetFileByIdError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(GetFileByIdError::NoFileWithThatId),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FileDeleteError {
    CannotDeleteRoot,
    FileDoesNotExist,
    InsufficientPermission,
}

impl From<LbError> for Error<FileDeleteError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
            LbErrorKind::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
            LbErrorKind::InsufficientPermission => UiError(FileDeleteError::InsufficientPermission),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ReadDocumentError {
    TreatedFolderAsDocument,
    FileDoesNotExist,
}

impl From<LbError> for Error<ReadDocumentError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNotDocument => UiError(ReadDocumentError::TreatedFolderAsDocument),
            LbErrorKind::FileNonexistent => UiError(ReadDocumentError::FileDoesNotExist),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<SaveDocumentToDiskError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNotDocument => {
                UiError(SaveDocumentToDiskError::TreatedFolderAsDocument)
            }
            LbErrorKind::FileNonexistent => UiError(SaveDocumentToDiskError::FileDoesNotExist),
            LbErrorKind::DiskPathInvalid => UiError(SaveDocumentToDiskError::BadPath),
            LbErrorKind::DiskPathTaken => UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk),
            _ => unexpected!("{:#?}", err),
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
    InsufficientPermission,
}

impl From<LbError> for Error<RenameFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(RenameFileError::FileDoesNotExist),
            LbErrorKind::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
            LbErrorKind::FileNameContainsSlash => UiError(RenameFileError::NewNameContainsSlash),
            LbErrorKind::PathTaken => UiError(RenameFileError::FileNameNotAvailable),
            LbErrorKind::RootModificationInvalid => UiError(RenameFileError::CannotRenameRoot),
            LbErrorKind::InsufficientPermission => UiError(RenameFileError::InsufficientPermission),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<MoveFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::RootModificationInvalid => UiError(MoveFileError::CannotMoveRoot),
            LbErrorKind::FileNotFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
            LbErrorKind::FileNonexistent => UiError(MoveFileError::FileDoesNotExist),
            LbErrorKind::FolderMovedIntoSelf => UiError(MoveFileError::FolderMovedIntoItself),
            LbErrorKind::FileParentNonexistent => UiError(MoveFileError::TargetParentDoesNotExist),
            LbErrorKind::PathTaken => UiError(MoveFileError::TargetParentHasChildNamedThat),
            LbErrorKind::LinkInSharedFolder => UiError(MoveFileError::LinkInSharedFolder),
            LbErrorKind::InsufficientPermission => UiError(MoveFileError::InsufficientPermission),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<ShareFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::RootModificationInvalid => UiError(ShareFileError::CannotShareRoot),
            LbErrorKind::FileNonexistent => UiError(ShareFileError::FileNonexistent),
            LbErrorKind::ShareAlreadyExists => UiError(ShareFileError::ShareAlreadyExists),
            LbErrorKind::LinkInSharedFolder => UiError(ShareFileError::LinkInSharedFolder),
            LbErrorKind::InsufficientPermission => UiError(ShareFileError::InsufficientPermission),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum DeletePendingShareError {
    FileNonexistent,
    ShareNonexistent,
}

impl From<LbError> for Error<DeletePendingShareError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(DeletePendingShareError::FileNonexistent),
            LbErrorKind::ShareNonexistent => UiError(DeletePendingShareError::ShareNonexistent),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<CreateLinkAtPathError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::PathContainsEmptyFileName => {
                UiError(CreateLinkAtPathError::PathContainsEmptyFile)
            }
            LbErrorKind::PathTaken => UiError(CreateLinkAtPathError::FileAlreadyExists),
            LbErrorKind::FileNotFolder => UiError(CreateLinkAtPathError::DocumentTreatedAsFolder),
            LbErrorKind::LinkInSharedFolder => UiError(CreateLinkAtPathError::LinkInSharedFolder),
            LbErrorKind::LinkTargetIsOwned => UiError(CreateLinkAtPathError::LinkTargetIsOwned),
            LbErrorKind::MultipleLinksToSameFile => {
                UiError(CreateLinkAtPathError::MultipleLinksToSameFile)
            }
            LbErrorKind::LinkTargetNonexistent => {
                UiError(CreateLinkAtPathError::LinkTargetNonexistent)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SyncAllError {
    Retry,
    ClientUpdateRequired,
    CouldNotReachServer,
}

impl From<LbError> for Error<SyncAllError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            // TODO figure out under what circumstances a user should retry a sync
            LbErrorKind::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum CalculateWorkError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<CalculateWorkError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetUsageError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<GetUsageError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetDrawingError {
    FolderTreatedAsDrawing,
    InvalidDrawing,
    FileDoesNotExist,
}

impl From<LbError> for Error<GetDrawingError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::DrawingInvalid => UiError(GetDrawingError::InvalidDrawing),
            LbErrorKind::FileNotDocument => UiError(GetDrawingError::FolderTreatedAsDrawing),
            LbErrorKind::FileNonexistent => UiError(GetDrawingError::FileDoesNotExist),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum SaveDrawingError {
    FileDoesNotExist,
    FolderTreatedAsDrawing,
    InvalidDrawing,
}

impl From<LbError> for Error<SaveDrawingError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::DrawingInvalid => UiError(SaveDrawingError::InvalidDrawing),
            LbErrorKind::FileNonexistent => UiError(SaveDrawingError::FileDoesNotExist),
            LbErrorKind::FileNotDocument => UiError(SaveDrawingError::FolderTreatedAsDrawing),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportDrawingError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    InvalidDrawing,
}

impl From<LbError> for Error<ExportDrawingError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::DrawingInvalid => UiError(ExportDrawingError::InvalidDrawing),
            LbErrorKind::FileNonexistent => UiError(ExportDrawingError::FileDoesNotExist),
            LbErrorKind::FileNotDocument => UiError(ExportDrawingError::FolderTreatedAsDrawing),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<ExportDrawingToDiskError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::DrawingInvalid => UiError(ExportDrawingToDiskError::InvalidDrawing),
            LbErrorKind::FileNonexistent => UiError(ExportDrawingToDiskError::FileDoesNotExist),
            LbErrorKind::FileNotDocument => {
                UiError(ExportDrawingToDiskError::FolderTreatedAsDrawing)
            }
            LbErrorKind::DiskPathInvalid => UiError(ExportDrawingToDiskError::BadPath),
            LbErrorKind::DiskPathTaken => {
                UiError(ExportDrawingToDiskError::FileAlreadyExistsInDisk)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ImportFileError {
    ParentDoesNotExist,
    DocumentTreatedAsFolder,
}

impl From<LbError> for Error<ImportFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(ImportFileError::ParentDoesNotExist),
            LbErrorKind::FileNotFolder => UiError(ImportFileError::DocumentTreatedAsFolder),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum ExportFileError {
    ParentDoesNotExist,
    DiskPathTaken,
    DiskPathInvalid,
}

impl From<LbError> for Error<ExportFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::FileNonexistent => UiError(ExportFileError::ParentDoesNotExist),
            LbErrorKind::DiskPathInvalid => UiError(ExportFileError::DiskPathInvalid),
            LbErrorKind::DiskPathTaken => UiError(ExportFileError::DiskPathTaken),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<UpgradeAccountStripeError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::OldCardDoesNotExist => {
                UiError(UpgradeAccountStripeError::OldCardDoesNotExist)
            }
            LbErrorKind::CardInvalidNumber => UiError(UpgradeAccountStripeError::InvalidCardNumber),
            LbErrorKind::CardInvalidExpYear => {
                UiError(UpgradeAccountStripeError::InvalidCardExpYear)
            }
            LbErrorKind::CardInvalidExpMonth => {
                UiError(UpgradeAccountStripeError::InvalidCardExpMonth)
            }
            LbErrorKind::CardInvalidCvc => UiError(UpgradeAccountStripeError::InvalidCardCvc),
            LbErrorKind::AlreadyPremium => UiError(UpgradeAccountStripeError::AlreadyPremium),
            LbErrorKind::ServerUnreachable => {
                UiError(UpgradeAccountStripeError::CouldNotReachServer)
            }
            LbErrorKind::CardDecline => UiError(UpgradeAccountStripeError::CardDecline),
            LbErrorKind::CardInsufficientFunds => {
                UiError(UpgradeAccountStripeError::CardHasInsufficientFunds)
            }
            LbErrorKind::TryAgain => UiError(UpgradeAccountStripeError::TryAgain),
            LbErrorKind::CardNotSupported => UiError(UpgradeAccountStripeError::CardNotSupported),
            LbErrorKind::CardExpired => UiError(UpgradeAccountStripeError::ExpiredCard),
            LbErrorKind::CurrentUsageIsMoreThanNewTier => {
                UiError(UpgradeAccountStripeError::CurrentUsageIsMoreThanNewTier)
            }
            LbErrorKind::ExistingRequestPending => {
                UiError(UpgradeAccountStripeError::ExistingRequestPending)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(UpgradeAccountStripeError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<UpgradeAccountGooglePlayError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AlreadyPremium => UiError(UpgradeAccountGooglePlayError::AlreadyPremium),
            LbErrorKind::InvalidAuthDetails => {
                UiError(UpgradeAccountGooglePlayError::InvalidAuthDetails)
            }
            LbErrorKind::ExistingRequestPending => {
                UiError(UpgradeAccountGooglePlayError::ExistingRequestPending)
            }
            LbErrorKind::ServerUnreachable => {
                UiError(UpgradeAccountGooglePlayError::CouldNotReachServer)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(UpgradeAccountGooglePlayError::ClientUpdateRequired)
            }
            LbErrorKind::AppStoreAccountAlreadyLinked => {
                UiError(UpgradeAccountGooglePlayError::AppStoreAccountAlreadyLinked)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<UpgradeAccountAppStoreError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::AlreadyPremium => UiError(UpgradeAccountAppStoreError::AlreadyPremium),
            LbErrorKind::InvalidPurchaseToken => {
                UiError(UpgradeAccountAppStoreError::InvalidPurchaseToken)
            }
            LbErrorKind::ExistingRequestPending => {
                UiError(UpgradeAccountAppStoreError::ExistingRequestPending)
            }
            LbErrorKind::ServerUnreachable => {
                UiError(UpgradeAccountAppStoreError::CouldNotReachServer)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(UpgradeAccountAppStoreError::ClientUpdateRequired)
            }
            LbErrorKind::AppStoreAccountAlreadyLinked => {
                UiError(UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<CancelSubscriptionError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::NotPremium => UiError(CancelSubscriptionError::NotPremium),
            LbErrorKind::AlreadyCanceled => UiError(CancelSubscriptionError::AlreadyCanceled),
            LbErrorKind::UsageIsOverFreeTierDataCap => {
                UiError(CancelSubscriptionError::UsageIsOverFreeTierDataCap)
            }
            LbErrorKind::ExistingRequestPending => {
                UiError(CancelSubscriptionError::ExistingRequestPending)
            }
            LbErrorKind::CannotCancelSubscriptionForAppStore => {
                UiError(CancelSubscriptionError::CannotCancelForAppStore)
            }
            LbErrorKind::ServerUnreachable => UiError(CancelSubscriptionError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => {
                UiError(CancelSubscriptionError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum GetSubscriptionInfoError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<GetSubscriptionInfoError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::ServerUnreachable => {
                UiError(GetSubscriptionInfoError::CouldNotReachServer)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(GetSubscriptionInfoError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum DeleteAccountError {
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<DeleteAccountError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::ServerUnreachable => UiError(DeleteAccountError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(DeleteAccountError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminDisappearAccount> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminDisappearAccount::InsufficientPermission)
            }
            LbErrorKind::UsernameNotFound => UiError(AdminDisappearAccount::UsernameNotFound),
            LbErrorKind::ServerUnreachable => UiError(AdminDisappearAccount::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminDisappearAccount::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminDisappearFileError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminDisappearFileError::InsufficientPermission)
            }
            LbErrorKind::FileNonexistent => UiError(AdminDisappearFileError::FileNotFound),
            LbErrorKind::ServerUnreachable => UiError(AdminDisappearFileError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminDisappearFileError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminServerValidateError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminServerValidateError::InsufficientPermission)
            }
            LbErrorKind::ServerUnreachable => {
                UiError(AdminServerValidateError::CouldNotReachServer)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminServerValidateError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminListUsersError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<AdminListUsersError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminListUsersError::InsufficientPermission)
            }
            LbErrorKind::ServerUnreachable => UiError(AdminListUsersError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(AdminListUsersError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum AdminRebuildIndexError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<AdminRebuildIndexError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminRebuildIndexError::InsufficientPermission)
            }
            LbErrorKind::ServerUnreachable => UiError(AdminRebuildIndexError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminRebuildIndexError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminGetAccountInfoError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminGetAccountInfoError::InsufficientPermission)
            }
            LbErrorKind::UsernameNotFound => UiError(AdminGetAccountInfoError::UsernameNotFound),
            LbErrorKind::ServerUnreachable => {
                UiError(AdminGetAccountInfoError::CouldNotReachServer)
            }
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminGetAccountInfoError::ClientUpdateRequired)
            }
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminFileInfoError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminFileInfoError::InsufficientPermission)
            }
            LbErrorKind::FileNonexistent => UiError(AdminFileInfoError::FileNotFound),
            LbErrorKind::ServerUnreachable => UiError(AdminFileInfoError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(AdminFileInfoError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
        }
    }
}

#[derive(Debug, Serialize, EnumIter)]
pub enum FeatureFlagError {
    InsufficientPermission,
    CouldNotReachServer,
    ClientUpdateRequired,
}

impl From<LbError> for Error<FeatureFlagError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(FeatureFlagError::InsufficientPermission)
            }
            LbErrorKind::ServerUnreachable => UiError(FeatureFlagError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => UiError(FeatureFlagError::ClientUpdateRequired),
            _ => unexpected!("{:#?}", err),
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

impl From<LbError> for Error<AdminSetUserTierError> {
    fn from(err: LbError) -> Self {
        match err.kind {
            LbErrorKind::InsufficientPermission => {
                UiError(AdminSetUserTierError::InsufficientPermission)
            }
            LbErrorKind::UsernameNotFound => UiError(AdminSetUserTierError::UsernameNotFound),
            LbErrorKind::ServerUnreachable => UiError(AdminSetUserTierError::CouldNotReachServer),
            LbErrorKind::ClientUpdateRequired => {
                UiError(AdminSetUserTierError::ClientUpdateRequired)
            }
            LbErrorKind::ExistingRequestPending => {
                UiError(AdminSetUserTierError::ExistingRequestPending)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}
