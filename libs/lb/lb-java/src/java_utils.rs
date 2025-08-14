use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JObject, JString, JThrowable, JValue};
use lb_rs::blocking::Lb;
use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::{LbErr, LbErrKind};

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

pub(crate) fn jbyte_array<'local>(env: &mut JNIEnv<'local>, bytes: Vec<u8>) -> JByteArray<'local> {
    let bytes: Vec<i8> = bytes.into_iter().map(|byte| byte as i8).collect();
    let jbytes = env.new_byte_array(bytes.len() as i32).unwrap();

    env.set_byte_array_region(&jbytes, 0, &bytes).unwrap();

    jbytes
}

pub(crate) fn rbyte_array(env: &JNIEnv, bytes: JByteArray) -> Vec<u8> {
    let len = env.get_array_length(&bytes).unwrap();
    let mut rbytes = vec![0i8; len as usize];

    env.get_byte_array_region(bytes, 0, &mut rbytes).unwrap();
    rbytes.into_iter().map(|b| b as u8).collect()
}

pub(crate) fn throw_err<'local>(env: &mut JNIEnv<'local>, err: LbErr) -> JObject<'local> {
    let j_err = env.find_class("net/lockbook/LbError").unwrap();

    let obj = env.alloc_object(j_err).unwrap();

    // msg
    let msg = jni_string(env, err.to_string());
    env.set_field(&obj, "msg", "Ljava/lang/String;", JValue::Object(&msg))
        .unwrap();

    // kind
    let enum_class = env.find_class("net/lockbook/LbError$LbEC").unwrap();

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
        LbErrKind::FileParentNonexistent => "FileParentNonexistent",
        LbErrKind::InsufficientPermission => "InsufficientPermission",
        LbErrKind::InvalidPurchaseToken => "InvalidPurchaseToken",
        LbErrKind::InvalidAuthDetails => "InvalidAuthDetails",
        LbErrKind::KeyPhraseInvalid => "KeyPhraseInvalid",
        LbErrKind::NotPremium => "NotPremium",
        LbErrKind::UsageIsOverDataCap => "UsageIsOverDataCap",
        LbErrKind::UsageIsOverFreeTierDataCap => "UsageIsOverFreeTierDataCap",
        LbErrKind::OldCardDoesNotExist => "OldCardDoesNotExist",
        LbErrKind::PathContainsEmptyFileName => "PathContainsEmptyFileName",
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
        LbErrKind::Validation(vf) => match vf {
            ValidationFailure::Cycle(_) => "FolderMovedIntoSelf",
            ValidationFailure::PathConflict(_) => "PathTaken",
            ValidationFailure::FileNameTooLong(_) => todo!(),
            ValidationFailure::NonFolderWithChildren(_) => todo!(),
            ValidationFailure::OwnedLink(_) => "LinkTargetIsOwned",
            ValidationFailure::BrokenLink(_) => "LinkTargetNonexistent",
            ValidationFailure::DuplicateLink { .. } => "MultipleLinksToSameFile",
            ValidationFailure::SharedLink { .. } => "LinkInSharedFolder",
            // todo: give this scenario it's own type
            ValidationFailure::DeletedFileUpdated(_) => "FileNonexistent",
            _ => "Unexpected",
        },
        LbErrKind::Unexpected(_) => "Unexpected",
        _ => "Unexpected",
    };
    let enum_constant = env
        .get_static_field(enum_class, name, "Lnet/lockbook/LbError$LbEC;")
        .unwrap()
        .l()
        .unwrap();
    env.set_field(&obj, "kind", "Lnet/lockbook/LbError$LbEC;", JValue::Object(&enum_constant))
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
