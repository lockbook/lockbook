use jni::{
    objects::{JClass, JObject, JString, JThrowable, JValue},
    JNIEnv,
};
use lb_rs::{
    blocking::Lb,
    model::errors::{LbErr, LbErrKind},
};

pub(crate) fn rstring<'local>(env: &mut JNIEnv<'local>, input: JString<'local>) -> String {
    env.get_string(&input).unwrap().to_str().unwrap().to_owned()
}

pub(crate) fn jni_string<'local>(env: &mut JNIEnv<'local>, input: String) -> JString<'local> {
    env.new_string(input).unwrap()
}

pub(crate) fn rlb<'local>(env: &mut JNIEnv<'local>, class: &JClass<'local>) -> &'local mut Lb {
    let ptr = env.get_static_field(class, "lb", "J").unwrap().j().unwrap();

    let ptr = ptr as *mut Lb;
    let ptr = unsafe { ptr.as_mut().unwrap() };
    ptr
}

pub(crate) fn throw_err<'local>(env: &mut JNIEnv<'local>, err: LbErr) -> JObject<'local> {
    let j_err = env.find_class("Lnet/lockbook/Err;").unwrap();

    let obj = env.alloc_object(j_err).unwrap();

    // msg
    let msg = jni_string(env, err.to_string());
    env.set_field(&obj, "msg", "Ljava/lang/String;", JValue::Object(&msg))
        .unwrap();

    // kind
    let enum_class = env.find_class("Lnet/lockbook/EKind;").unwrap();

    let name = match err.kind {
        LbErrKind::AccountExists => "AccountExists",
        LbErrKind::AccountNonexistent => "AccountNonexistent",
        LbErrKind::AccountStringCorrupted => "AccountStringCorrupted",
        LbErrKind::AlreadyCanceled => "AlreadyCanceled",
        LbErrKind::AlreadyPremium => "AlreadyPremium",
        LbErrKind::AppStoreAccountAlreadyLinked => "AppStoreAccountAlreadyLinked",
        LbErrKind::AlreadySyncing => "AlreadySyncing",
        LbErrKind::CannotCancelSubscriptionForAppStore => "CannotCancelSubscriptionForAppStore",
        LbErrKind::CardDecline => "CardDecline",
        LbErrKind::CardExpired => "CardExpired",
        LbErrKind::CardInsufficientFunds => "CardInsufficientFunds",
        LbErrKind::CardInvalidCvc => "CardInvalidCvc",
        LbErrKind::CardInvalidExpMonth => "CardInvalidExpMonth",
        LbErrKind::CardInvalidExpYear => "CardInvalidExpYear",
        LbErrKind::CardInvalidNumber => "CardInvalidNumber",
        LbErrKind::CardNotSupported => "CardNotSupported",
        LbErrKind::ClientUpdateRequired => "ClientUpdateRequired",
        LbErrKind::CurrentUsageIsMoreThanNewTier => "CurrentUsageIsMoreThanNewTier",
        LbErrKind::DiskPathInvalid => "DiskPathInvalid",
        LbErrKind::DiskPathTaken => "DiskPathTaken",
        LbErrKind::DrawingInvalid => "DrawingInvalid",
        LbErrKind::ExistingRequestPending => "ExistingRequestPending",
        LbErrKind::FileNameContainsSlash => "FileNameContainsSlash",
        LbErrKind::FileNameTooLong => "FileNameTooLong",
        LbErrKind::FileNameEmpty => "FileNameEmpty",
        LbErrKind::FileNonexistent => "FileNonexistent",
        LbErrKind::FileNotDocument => "FileNotDocument",
        LbErrKind::FileNotFolder => "FileNotFolder",
        LbErrKind::FileParentNonexistent => "FileParentNonexistent",
        LbErrKind::FolderMovedIntoSelf => "FolderMovedIntoSelf",
        LbErrKind::InsufficientPermission => "InsufficientPermission",
        LbErrKind::InvalidPurchaseToken => "InvalidPurchaseToken",
        LbErrKind::InvalidAuthDetails => "InvalidAuthDetails",
        LbErrKind::KeyPhraseInvalid => "KeyPhraseInvalid",
        LbErrKind::LinkInSharedFolder => "LinkInSharedFolder",
        LbErrKind::LinkTargetIsOwned => "LinkTargetIsOwned",
        LbErrKind::LinkTargetNonexistent => "LinkTargetNonexistent",
        LbErrKind::MultipleLinksToSameFile => "MultipleLinksToSameFile",
        LbErrKind::NotPremium => "NotPremium",
        LbErrKind::UsageIsOverDataCap => "UsageIsOverDataCap",
        LbErrKind::UsageIsOverFreeTierDataCap => "UsageIsOverFreeTierDataCap",
        LbErrKind::OldCardDoesNotExist => "OldCardDoesNotExist",
        LbErrKind::PathContainsEmptyFileName => "PathContainsEmptyFileName",
        LbErrKind::PathTaken => "PathTaken",
        LbErrKind::RootModificationInvalid => "RootModificationInvalid",
        LbErrKind::RootNonexistent => "RootNonexistent",
        LbErrKind::ServerDisabled => "ServerDisabled",
        LbErrKind::ServerUnreachable => "ServerUnreachable",
        LbErrKind::ShareAlreadyExists => "ShareAlreadyExists",
        LbErrKind::ShareNonexistent => "ShareNonexistent",
        LbErrKind::TryAgain => "TryAgain",
        LbErrKind::UsernameInvalid => "UsernameInvalid",
        LbErrKind::UsernameNotFound => "UsernameNotFound",
        LbErrKind::UsernamePublicKeyMismatch => "UsernamePublicKeyMismatch",
        LbErrKind::UsernameTaken => "UsernameTaken",
        LbErrKind::ReReadRequired => "ReReadRequired",
        LbErrKind::Unexpected(_) => "Unexpected",
    };
    let enum_constant = env
        .get_static_field(enum_class, name, "Lnet/lockbook/EKind;")
        .unwrap()
        .l()
        .unwrap();
    env.set_field(&obj, "kind", "Lnet/lockbook/EKind;", JValue::Object(&enum_constant))
        .unwrap();

    // trace
    if let Some(trace) = err.backtrace {
        let msg = jni_string(env, trace.to_string());
        env.set_field(&obj, "trace", "Ljava/lang/String;", JValue::Object(&msg))
            .unwrap();
    }

    env.throw(JThrowable::from(obj)).unwrap();

    JObject::null()
}
