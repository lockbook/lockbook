use std::panic;

use jni::{
    objects::{JByteArray, JClass, JObject, JObjectArray, JString, JThrowable, JValue}, sys::{jbyte, jbyteArray, jlong}, JNIEnv
};
use lb_rs::{
    blocking::Lb,
    model::{errors::{LbErr, LbErrKind}, file::{File, ShareMode}, file_metadata::FileType},
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

pub(crate) fn jbyte_array<'local>(env: &mut JNIEnv<'local>, bytes: Vec<u8>) -> JByteArray<'local> {
    let bytes: Vec<i8> = bytes.into_iter().map(|byte| byte as i8).collect();
    let jbytes = env.new_byte_array(bytes.len() as i32).unwrap();
    
    env.set_byte_array_region(&jbytes, 0, &bytes).unwrap();

    jbytes
}

pub(crate) fn jfiles<'local>(env: &mut JNIEnv<'local>, rust_files: Vec<File>) -> JObjectArray<'local> {
    let file_class = env.find_class("Lnet/lockbook/File;").unwrap();
    let obj = env.new_object_array(rust_files.len() as i32, file_class, JObject::null()).unwrap();

    for (i, rust_file) in rust_files.iter().enumerate() {
        // file
        let file = jfile(env, rust_file.clone());
        env.set_object_array_element(&obj, i as i32, file).unwrap();
    }

    obj
}

pub(crate) fn jfile<'local>(env: &mut JNIEnv<'local>, file: File) -> JObject<'local> {
    let file_class = env.find_class("Lnet/lockbook/File;").unwrap();
    let obj = env.alloc_object(file_class).unwrap();
    
    // id
    let id = jni_string(env, file.id.to_string());
    env.set_field(&obj, "id", "Ljava/lang/String;", JValue::Object(&id)).unwrap();

    // parent
    let parent = jni_string(env, file.parent.to_string());
    env.set_field(&obj, "parent", "Ljava/lang/String;", JValue::Object(&parent)).unwrap();

    // name
    let name = jni_string(env, file.name);
    env.set_field(&obj, "name", "Ljava/lang/String;", JValue::Object(&name)).unwrap();
    
    // file type
    let enum_class = env.find_class("Lnet/lockbook/File$FileType;").unwrap();
    let filetype_name = match file.file_type {
        FileType::Document => "Document",
        FileType::Folder => "Folder",
        FileType::Link { .. } => panic!("did not expect link file type!")
    };
    let enum_constant = env
        .get_static_field(enum_class, filetype_name, "Lnet/lockbook/File$FileType;")
        .unwrap()
        .l()
        .unwrap();

    env.set_field(&obj, "fileType", "Lnet/lockbook/File$FileType;", JValue::Object(&enum_constant))
        .unwrap();

    // last modified
    env.set_field(&obj, "lastModified", "J", JValue::Long(file.last_modified as jlong)).unwrap();

    // last modified by
    let last_modified_by = jni_string(env, file.last_modified_by);
    env.set_field(&obj, "lastModifiedBy", "Ljava/lang/String;", JValue::Object(&last_modified_by)).unwrap();

    let share_class = env.find_class("Lnet/lockbook/File$Share;").unwrap();
    let share_mode_class = env.find_class("Lnet/lockbook/File$ShareMode;").unwrap();

    // shares
    let shares_array = env.new_object_array(file.shares.len() as i32, &share_class, JObject::null()).unwrap();

    for (i, share) in file.shares.iter().enumerate() {
        // Allocate Share object
        let jshare = env.alloc_object(&share_class).unwrap();

        // mode
        let mode_name = match share.mode {
            ShareMode::Write => "Write",
            ShareMode::Read => "Read",
        };
        let mode_constant = env
            .get_static_field(&share_mode_class, mode_name, "Lnet/lockbook/File$ShareMode;")
            .unwrap()
            .l()
            .unwrap();
        env.set_field(&jshare, "mode", "Lnet/lockbook/File$ShareMode;", JValue::Object(&mode_constant)).unwrap();

        // shared by
        let shared_by = jni_string(env, share.shared_by.clone());
        env.set_field(&jshare, "sharedBy", "Ljava/lang/String;", JValue::Object(&shared_by)).unwrap();

        // shared with
        let shared_with = jni_string(env, share.shared_with.clone());
        env.set_field(&jshare, "sharedWith", "Ljava/lang/String;", JValue::Object(&shared_with)).unwrap();
        env.set_object_array_element(&shares_array, i as i32, jshare).unwrap();
    }

    env.set_field(&obj, "shares", "[Lnet/lockbook/File$Share;", JValue::Object(&shares_array)).unwrap();

    obj
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
