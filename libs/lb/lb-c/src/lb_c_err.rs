use std::{
    ffi::{c_char, CString},
    ptr,
};

use lb_rs::model::errors::{LbErr, LbErrKind};

use crate::ffi_utils::cstring;

#[repr(C)]
pub struct LbFfiErr {
    pub code: LbEC,
    pub msg: *mut c_char,
    pub trace: *mut c_char,
}

#[repr(C)]
pub enum LbEC {
    Success = 0,
    Unexpected,
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
    ReReadRequired,
    ServerDisabled,
    ServerUnreachable,
    ShareAlreadyExists,
    ShareNonexistent,
    TryAgain,
    UsernameInvalid,
    UsernameNotFound,
    UsernamePublicKeyMismatch,
    UsernameTaken,
}

impl From<LbErr> for LbFfiErr {
    fn from(value: LbErr) -> Self {
        let code = (&value.kind).into();
        let msg = value.to_string();
        let msg = CString::new(msg).unwrap().into_raw();

        let trace = match value.backtrace {
            Some(bt) => cstring(bt.to_string()),
            None => ptr::null_mut(),
        };

        Self { code, msg, trace }
    }
}

impl From<&LbErrKind> for LbEC {
    fn from(value: &LbErrKind) -> Self {
        match value {
            LbErrKind::AccountExists => Self::AccountExists,
            LbErrKind::AccountNonexistent => Self::AccountNonexistent,
            LbErrKind::AccountStringCorrupted => Self::AccountStringCorrupted,
            LbErrKind::AlreadyCanceled => Self::AlreadyCanceled,
            LbErrKind::AlreadyPremium => Self::AlreadyPremium,
            LbErrKind::AppStoreAccountAlreadyLinked => Self::AppStoreAccountAlreadyLinked,
            LbErrKind::AlreadySyncing => Self::AlreadySyncing,
            LbErrKind::CannotCancelSubscriptionForAppStore => {
                Self::CannotCancelSubscriptionForAppStore
            }
            LbErrKind::CardDecline => Self::CardDecline,
            LbErrKind::CardExpired => Self::CardExpired,
            LbErrKind::CardInsufficientFunds => Self::CardInsufficientFunds,
            LbErrKind::CardInvalidCvc => Self::CardInvalidCvc,
            LbErrKind::CardInvalidExpMonth => Self::CardInvalidExpMonth,
            LbErrKind::CardInvalidExpYear => Self::CardInvalidExpYear,
            LbErrKind::CardInvalidNumber => Self::CardInvalidNumber,
            LbErrKind::CardNotSupported => Self::CardNotSupported,
            LbErrKind::ClientUpdateRequired => Self::ClientUpdateRequired,
            LbErrKind::CurrentUsageIsMoreThanNewTier => Self::CurrentUsageIsMoreThanNewTier,
            LbErrKind::DiskPathInvalid => Self::DiskPathInvalid,
            LbErrKind::DiskPathTaken => Self::DiskPathTaken,
            LbErrKind::DrawingInvalid => Self::DrawingInvalid,
            LbErrKind::ExistingRequestPending => Self::ExistingRequestPending,
            LbErrKind::FileNameContainsSlash => Self::FileNameContainsSlash,
            LbErrKind::FileNameTooLong => Self::FileNameTooLong,
            LbErrKind::FileNameEmpty => Self::FileNameEmpty,
            LbErrKind::FileNonexistent => Self::FileNonexistent,
            LbErrKind::FileNotDocument => Self::FileNotDocument,
            LbErrKind::FileNotFolder => Self::FileNotFolder,
            LbErrKind::FileParentNonexistent => Self::FileParentNonexistent,
            LbErrKind::FolderMovedIntoSelf => Self::FolderMovedIntoSelf,
            LbErrKind::InsufficientPermission => Self::InsufficientPermission,
            LbErrKind::InvalidPurchaseToken => Self::InvalidPurchaseToken,
            LbErrKind::InvalidAuthDetails => Self::InvalidAuthDetails,
            LbErrKind::KeyPhraseInvalid => Self::KeyPhraseInvalid,
            LbErrKind::LinkInSharedFolder => Self::LinkInSharedFolder,
            LbErrKind::LinkTargetIsOwned => Self::LinkTargetIsOwned,
            LbErrKind::LinkTargetNonexistent => Self::LinkTargetNonexistent,
            LbErrKind::MultipleLinksToSameFile => Self::MultipleLinksToSameFile,
            LbErrKind::NotPremium => Self::NotPremium,
            LbErrKind::UsageIsOverDataCap => Self::UsageIsOverDataCap,
            LbErrKind::UsageIsOverFreeTierDataCap => Self::UsageIsOverFreeTierDataCap,
            LbErrKind::OldCardDoesNotExist => Self::OldCardDoesNotExist,
            LbErrKind::PathContainsEmptyFileName => Self::PathContainsEmptyFileName,
            LbErrKind::PathTaken => Self::PathTaken,
            LbErrKind::RootModificationInvalid => Self::RootModificationInvalid,
            LbErrKind::RootNonexistent => Self::RootNonexistent,
            LbErrKind::ServerDisabled => Self::ServerDisabled,
            LbErrKind::ServerUnreachable => Self::ServerUnreachable,
            LbErrKind::ShareAlreadyExists => Self::ShareAlreadyExists,
            LbErrKind::ShareNonexistent => Self::ShareNonexistent,
            LbErrKind::TryAgain => Self::TryAgain,
            LbErrKind::UsernameInvalid => Self::UsernameInvalid,
            LbErrKind::UsernameNotFound => Self::UsernameNotFound,
            LbErrKind::UsernamePublicKeyMismatch => Self::UsernamePublicKeyMismatch,
            LbErrKind::UsernameTaken => Self::UsernameTaken,
            LbErrKind::Unexpected(_) => Self::ReReadRequired,
            LbErrKind::ReReadRequired => todo!(),
        }
    }
}