use serde::Serialize;
use strum_macros::EnumIter;

use lockbook_core::{CoreError, LbError};

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
            CoreError::AccountStringCorrupted => UiError(ImportError::AccountStringCorrupted),
            CoreError::AccountExists => UiError(ImportError::AccountExistsAlready),
            CoreError::UsernamePublicKeyMismatch => UiError(ImportError::UsernamePKMismatch),
            CoreError::ServerUnreachable => UiError(ImportError::CouldNotReachServer),
            CoreError::AccountNonexistent => UiError(ImportError::AccountDoesNotExist),
            CoreError::ClientUpdateRequired => UiError(ImportError::ClientUpdateRequired),
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
            CoreError::AccountNonexistent => UiError(AccountExportError::NoAccount),
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
            CoreError::AccountNonexistent => UiError(GetAccountError::NoAccount),
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
            CoreError::PathContainsEmptyFileName => {
                UiError(CreateFileAtPathError::PathContainsEmptyFile)
            }
            CoreError::RootNonexistent => UiError(CreateFileAtPathError::NoRoot),
            CoreError::PathTaken => UiError(CreateFileAtPathError::FileAlreadyExists),
            CoreError::FileNotFolder => UiError(CreateFileAtPathError::DocumentTreatedAsFolder),
            CoreError::InsufficientPermission => {
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
            CoreError::FileNonexistent => UiError(GetFileByPathError::NoFileAtThatPath),
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
            CoreError::FileNonexistent => UiError(WriteToDocumentError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(WriteToDocumentError::FolderTreatedAsDocument),
            CoreError::InsufficientPermission => {
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
            CoreError::RootNonexistent => UiError(GetRootError::NoRoot),
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
            CoreError::FileNonexistent => UiError(GetAndGetChildrenError::FileDoesNotExist),
            CoreError::FileNotFolder => UiError(GetAndGetChildrenError::DocumentTreatedAsFolder),
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
            CoreError::FileNonexistent => UiError(GetFileByIdError::NoFileWithThatId),
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
            CoreError::RootModificationInvalid => UiError(FileDeleteError::CannotDeleteRoot),
            CoreError::FileNonexistent => UiError(FileDeleteError::FileDoesNotExist),
            CoreError::InsufficientPermission => UiError(FileDeleteError::InsufficientPermission),
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
            CoreError::FileNotDocument => UiError(ReadDocumentError::TreatedFolderAsDocument),
            CoreError::FileNonexistent => UiError(ReadDocumentError::FileDoesNotExist),
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
            CoreError::FileNotDocument => UiError(SaveDocumentToDiskError::TreatedFolderAsDocument),
            CoreError::FileNonexistent => UiError(SaveDocumentToDiskError::FileDoesNotExist),
            CoreError::DiskPathInvalid => UiError(SaveDocumentToDiskError::BadPath),
            CoreError::DiskPathTaken => UiError(SaveDocumentToDiskError::FileAlreadyExistsInDisk),
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
            CoreError::FileNonexistent => UiError(RenameFileError::FileDoesNotExist),
            CoreError::FileNameEmpty => UiError(RenameFileError::NewNameEmpty),
            CoreError::FileNameContainsSlash => UiError(RenameFileError::NewNameContainsSlash),
            CoreError::PathTaken => UiError(RenameFileError::FileNameNotAvailable),
            CoreError::RootModificationInvalid => UiError(RenameFileError::CannotRenameRoot),
            CoreError::InsufficientPermission => UiError(RenameFileError::InsufficientPermission),
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
            CoreError::RootModificationInvalid => UiError(MoveFileError::CannotMoveRoot),
            CoreError::FileNotFolder => UiError(MoveFileError::DocumentTreatedAsFolder),
            CoreError::FileNonexistent => UiError(MoveFileError::FileDoesNotExist),
            CoreError::FolderMovedIntoSelf => UiError(MoveFileError::FolderMovedIntoItself),
            CoreError::FileParentNonexistent => UiError(MoveFileError::TargetParentDoesNotExist),
            CoreError::PathTaken => UiError(MoveFileError::TargetParentHasChildNamedThat),
            CoreError::LinkInSharedFolder => UiError(MoveFileError::LinkInSharedFolder),
            CoreError::InsufficientPermission => UiError(MoveFileError::InsufficientPermission),
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
            CoreError::RootModificationInvalid => UiError(ShareFileError::CannotShareRoot),
            CoreError::FileNonexistent => UiError(ShareFileError::FileNonexistent),
            CoreError::ShareAlreadyExists => UiError(ShareFileError::ShareAlreadyExists),
            CoreError::LinkInSharedFolder => UiError(ShareFileError::LinkInSharedFolder),
            CoreError::InsufficientPermission => UiError(ShareFileError::InsufficientPermission),
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
            CoreError::FileNonexistent => UiError(DeletePendingShareError::FileNonexistent),
            CoreError::ShareNonexistent => UiError(DeletePendingShareError::ShareNonexistent),
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
            CoreError::ServerUnreachable => UiError(SyncAllError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(SyncAllError::ClientUpdateRequired),
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
            CoreError::ServerUnreachable => UiError(CalculateWorkError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(CalculateWorkError::ClientUpdateRequired),
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
            CoreError::ServerUnreachable => UiError(GetUsageError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(GetUsageError::ClientUpdateRequired),
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
            CoreError::DrawingInvalid => UiError(GetDrawingError::InvalidDrawing),
            CoreError::FileNotDocument => UiError(GetDrawingError::FolderTreatedAsDrawing),
            CoreError::FileNonexistent => UiError(GetDrawingError::FileDoesNotExist),
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
            CoreError::DrawingInvalid => UiError(SaveDrawingError::InvalidDrawing),
            CoreError::FileNonexistent => UiError(SaveDrawingError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(SaveDrawingError::FolderTreatedAsDrawing),
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
            CoreError::DrawingInvalid => UiError(ExportDrawingError::InvalidDrawing),
            CoreError::FileNonexistent => UiError(ExportDrawingError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(ExportDrawingError::FolderTreatedAsDrawing),
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
            CoreError::DrawingInvalid => UiError(ExportDrawingToDiskError::InvalidDrawing),
            CoreError::FileNonexistent => UiError(ExportDrawingToDiskError::FileDoesNotExist),
            CoreError::FileNotDocument => UiError(ExportDrawingToDiskError::FolderTreatedAsDrawing),
            CoreError::DiskPathInvalid => UiError(ExportDrawingToDiskError::BadPath),
            CoreError::DiskPathTaken => UiError(ExportDrawingToDiskError::FileAlreadyExistsInDisk),
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
            CoreError::FileNonexistent => UiError(ImportFileError::ParentDoesNotExist),
            CoreError::FileNotFolder => UiError(ImportFileError::DocumentTreatedAsFolder),
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
            CoreError::FileNonexistent => UiError(ExportFileError::ParentDoesNotExist),
            CoreError::DiskPathInvalid => UiError(ExportFileError::DiskPathInvalid),
            CoreError::DiskPathTaken => UiError(ExportFileError::DiskPathTaken),
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
            CoreError::OldCardDoesNotExist => {
                UiError(UpgradeAccountStripeError::OldCardDoesNotExist)
            }
            CoreError::CardInvalidNumber => UiError(UpgradeAccountStripeError::InvalidCardNumber),
            CoreError::CardInvalidExpYear => UiError(UpgradeAccountStripeError::InvalidCardExpYear),
            CoreError::CardInvalidExpMonth => {
                UiError(UpgradeAccountStripeError::InvalidCardExpMonth)
            }
            CoreError::CardInvalidCvc => UiError(UpgradeAccountStripeError::InvalidCardCvc),
            CoreError::AlreadyPremium => UiError(UpgradeAccountStripeError::AlreadyPremium),
            CoreError::ServerUnreachable => UiError(UpgradeAccountStripeError::CouldNotReachServer),
            CoreError::CardDecline => UiError(UpgradeAccountStripeError::CardDecline),
            CoreError::CardInsufficientFunds => {
                UiError(UpgradeAccountStripeError::CardHasInsufficientFunds)
            }
            CoreError::TryAgain => UiError(UpgradeAccountStripeError::TryAgain),
            CoreError::CardNotSupported => UiError(UpgradeAccountStripeError::CardNotSupported),
            CoreError::CardExpired => UiError(UpgradeAccountStripeError::ExpiredCard),
            CoreError::CurrentUsageIsMoreThanNewTier => {
                UiError(UpgradeAccountStripeError::CurrentUsageIsMoreThanNewTier)
            }
            CoreError::ExistingRequestPending => {
                UiError(UpgradeAccountStripeError::ExistingRequestPending)
            }
            CoreError::ClientUpdateRequired => {
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
            CoreError::ServerUnreachable => UiError(GetSubscriptionInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
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
            CoreError::ServerUnreachable => UiError(DeleteAccountError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(DeleteAccountError::ClientUpdateRequired),
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
            CoreError::InsufficientPermission => {
                UiError(AdminDisappearAccount::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminDisappearAccount::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminDisappearAccount::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminDisappearAccount::ClientUpdateRequired),
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
            CoreError::InsufficientPermission => {
                UiError(AdminDisappearFileError::InsufficientPermission)
            }
            CoreError::FileNonexistent => UiError(AdminDisappearFileError::FileNotFound),
            CoreError::ServerUnreachable => UiError(AdminDisappearFileError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
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
            CoreError::InsufficientPermission => {
                UiError(AdminServerValidateError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminServerValidateError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
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
            CoreError::InsufficientPermission => {
                UiError(AdminListUsersError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminListUsersError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminListUsersError::ClientUpdateRequired),
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
            CoreError::InsufficientPermission => {
                UiError(AdminRebuildIndexError::InsufficientPermission)
            }
            CoreError::ServerUnreachable => UiError(AdminRebuildIndexError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
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
            CoreError::InsufficientPermission => {
                UiError(AdminGetAccountInfoError::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminGetAccountInfoError::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminGetAccountInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => {
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
            CoreError::InsufficientPermission => {
                UiError(AdminFileInfoError::InsufficientPermission)
            }
            CoreError::FileNonexistent => UiError(AdminFileInfoError::FileNotFound),
            CoreError::ServerUnreachable => UiError(AdminFileInfoError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminFileInfoError::ClientUpdateRequired),
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
            CoreError::InsufficientPermission => UiError(FeatureFlagError::InsufficientPermission),
            CoreError::ServerUnreachable => UiError(FeatureFlagError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(FeatureFlagError::ClientUpdateRequired),
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
            CoreError::InsufficientPermission => {
                UiError(AdminSetUserTierError::InsufficientPermission)
            }
            CoreError::UsernameNotFound => UiError(AdminSetUserTierError::UsernameNotFound),
            CoreError::ServerUnreachable => UiError(AdminSetUserTierError::CouldNotReachServer),
            CoreError::ClientUpdateRequired => UiError(AdminSetUserTierError::ClientUpdateRequired),
            CoreError::ExistingRequestPending => {
                UiError(AdminSetUserTierError::ExistingRequestPending)
            }
            _ => unexpected!("{:#?}", err),
        }
    }
}
