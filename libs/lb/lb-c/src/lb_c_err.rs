use std::ffi::{c_char, CString};

use lb_rs::model::errors::{LbErr, LbErrKind};

#[repr(C)]
pub struct LbFfiErr {
    code: u16,
    msg: *mut c_char,
    trace: *mut c_char,
}

impl From<LbErr> for LbFfiErr {
    fn from(value: LbErr) -> Self {
        let code = match value.kind {
            LbErrKind::AccountExists => 1,
            LbErrKind::AccountNonexistent => 2,
            LbErrKind::AccountStringCorrupted => 3,
            LbErrKind::AlreadyCanceled => 4,
            LbErrKind::AlreadyPremium => 5,
            LbErrKind::AppStoreAccountAlreadyLinked => 6,
            LbErrKind::AlreadySyncing => 7,
            LbErrKind::CannotCancelSubscriptionForAppStore => 8,
            LbErrKind::CardDecline => 9,
            LbErrKind::CardExpired => 10,
            LbErrKind::CardInsufficientFunds => 11,
            LbErrKind::CardInvalidCvc => 12,
            LbErrKind::CardInvalidExpMonth => 13,
            LbErrKind::CardInvalidExpYear => 14,
            LbErrKind::CardInvalidNumber => 15,
            LbErrKind::CardNotSupported => 16,
            LbErrKind::ClientUpdateRequired => 17,
            LbErrKind::CurrentUsageIsMoreThanNewTier => 18,
            LbErrKind::DiskPathInvalid => 19,
            LbErrKind::DiskPathTaken => 20,
            LbErrKind::DrawingInvalid => 21,
            LbErrKind::ExistingRequestPending => 22,
            LbErrKind::FileNameContainsSlash => 23,
            LbErrKind::FileNameTooLong => 24,
            LbErrKind::FileNameEmpty => 25,
            LbErrKind::FileNonexistent => 26,
            LbErrKind::FileNotDocument => 27,
            LbErrKind::FileNotFolder => 28,
            LbErrKind::FileParentNonexistent => 29,
            LbErrKind::FolderMovedIntoSelf => 30,
            LbErrKind::InsufficientPermission => 31,
            LbErrKind::InvalidPurchaseToken => 32,
            LbErrKind::InvalidAuthDetails => 33,
            LbErrKind::KeyPhraseInvalid => 34,
            LbErrKind::LinkInSharedFolder => 35,
            LbErrKind::LinkTargetIsOwned => 36,
            LbErrKind::LinkTargetNonexistent => 37,
            LbErrKind::MultipleLinksToSameFile => 38,
            LbErrKind::NotPremium => 39,
            LbErrKind::UsageIsOverDataCap => 40,
            LbErrKind::UsageIsOverFreeTierDataCap => 41,
            LbErrKind::OldCardDoesNotExist => 42,
            LbErrKind::PathContainsEmptyFileName => 43,
            LbErrKind::PathTaken => 44,
            LbErrKind::RootModificationInvalid => 45,
            LbErrKind::RootNonexistent => 46,
            LbErrKind::ServerDisabled => 47,
            LbErrKind::ServerUnreachable => 48,
            LbErrKind::ShareAlreadyExists => 49,
            LbErrKind::ShareNonexistent => 50,
            LbErrKind::TryAgain => 51,
            LbErrKind::UsernameInvalid => 52,
            LbErrKind::UsernameNotFound => 53,
            LbErrKind::UsernamePublicKeyMismatch => 54,
            LbErrKind::UsernameTaken => 55,
            LbErrKind::Unexpected(_) => u16::max_value(),
        };

        let msg = value.to_string();
        let msg = CString::new(msg).unwrap().into_raw();

        let trace = match value.backtrace {
            Some(bt) => bt.to_string(),
            None => todo!(),
        };
        
        Self { code, msg, trace: todo!() }
    }
}

