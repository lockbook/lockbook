mod java_utils;

use std::str::FromStr;

use java_utils::{jbyte_array, jni_string, rbyte_array, rlb, rstring, throw_err};
use jni::{
    objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue},
    sys::{jboolean, jbyteArray, jlong, jobject, jobjectArray, jstring},
    JNIEnv,
};
use lb_rs::{
    blocking::{ChronoHumanDuration, Duration, Lb},
    model::{
        account::Account,
        clock,
        core_config::Config,
        file::{File, ShareMode},
        file_metadata::FileType,
    },
    service::{
        sync::SyncProgress,
        usage::{UsageItemMetric, UsageMetrics},
    },
    Uuid, DEFAULT_API_LOCATION,
};

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_init<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, input: JString<'local>,
) {
    let writeable_path = rstring(&mut env, input);
    let config = Config { logs: true, colored_logs: false, writeable_path };

    match Lb::init(config) {
        Ok(lb) => {
            let ptr = Box::into_raw(Box::new(lb)) as jlong;
            let field_id = env.get_static_field_id(&class, "lb", "J").unwrap();

            env.set_static_field(&class, field_id, jni::objects::JValueGen::Long(ptr))
                .unwrap();
        }
        Err(err) => {
            throw_err(&mut env, err);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_createAccount<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, uname: JString<'local>,
    api_url: JString<'local>, welcome_doc: jboolean,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let uname = rstring(&mut env, uname);
    let api_url = if api_url.is_null() {
        DEFAULT_API_LOCATION.to_string()
    } else {
        rstring(&mut env, api_url)
    };
    let welcome_doc = welcome_doc != 0;

    match lb.create_account(&uname, &api_url, welcome_doc) {
        Ok(account) => j_account(&mut env, account),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_importAccount<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, key: JString<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let key = rstring(&mut env, key);

    // todo: deal with None, check for null
    match lb.import_account(&key, None) {
        Ok(account) => j_account(&mut env, account),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

fn j_account<'local>(env: &mut JNIEnv<'local>, account: Account) -> JObject<'local> {
    let obj = env.find_class("Lnet/lockbook/Account;").unwrap();
    let obj = env.alloc_object(obj).unwrap();

    let uname = jni_string(env, account.username);
    let api_url = jni_string(env, account.api_url);

    env.set_field(&obj, "uname", "Ljava/lang/String;", JValue::Object(&uname))
        .unwrap();
    env.set_field(&obj, "apiUrl", "Ljava/lang/String;", JValue::Object(&api_url))
        .unwrap();

    obj
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getAccount<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    match lb.get_account() {
        Ok(account) => j_account(&mut env, account.clone()),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_exportAccountPrivateKey<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.export_account_private_key() {
        Ok(account) => jni_string(&mut env, account).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_exportAccountPhrase<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.export_account_phrase() {
        Ok(account) => jni_string(&mut env, account).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_exportAccountQR<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jbyteArray {
    let lb = rlb(&mut env, &class);

    match lb.export_account_qr() {
        Ok(qr) => jbyte_array(&mut env, qr).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

fn jfile<'local>(env: &mut JNIEnv<'local>, file: File) -> JObject<'local> {
    let file_class = env.find_class("Lnet/lockbook/File;").unwrap();
    let obj = env.alloc_object(file_class).unwrap();

    // id
    let id = jni_string(env, file.id.to_string());
    env.set_field(&obj, "id", "Ljava/lang/String;", JValue::Object(&id))
        .unwrap();

    // parent
    let parent = jni_string(env, file.parent.to_string());
    env.set_field(&obj, "parent", "Ljava/lang/String;", JValue::Object(&parent))
        .unwrap();

    // name
    let name = jni_string(env, file.name);
    env.set_field(&obj, "name", "Ljava/lang/String;", JValue::Object(&name))
        .unwrap();

    // file type
    let enum_class = env.find_class("Lnet/lockbook/File$FileType;").unwrap();
    let filetype_name = match file.file_type {
        FileType::Document => "Document",
        FileType::Folder => "Folder",
        FileType::Link { .. } => panic!("did not expect link file type!"),
    };
    let enum_constant = env
        .get_static_field(enum_class, filetype_name, "Lnet/lockbook/File$FileType;")
        .unwrap()
        .l()
        .unwrap();

    env.set_field(&obj, "fileType", "Lnet/lockbook/File$FileType;", JValue::Object(&enum_constant))
        .unwrap();

    // last modified
    env.set_field(&obj, "lastModified", "J", JValue::Long(file.last_modified as jlong))
        .unwrap();

    // last modified by
    let last_modified_by = jni_string(env, file.last_modified_by);
    env.set_field(&obj, "lastModifiedBy", "Ljava/lang/String;", JValue::Object(&last_modified_by))
        .unwrap();

    let share_class = env.find_class("Lnet/lockbook/File$Share;").unwrap();
    let share_mode_class = env.find_class("Lnet/lockbook/File$ShareMode;").unwrap();

    // shares
    let shares_array = env
        .new_object_array(file.shares.len() as i32, &share_class, JObject::null())
        .unwrap();

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
        env.set_field(
            &jshare,
            "mode",
            "Lnet/lockbook/File$ShareMode;",
            JValue::Object(&mode_constant),
        )
        .unwrap();

        // shared by
        let shared_by = jni_string(env, share.shared_by.clone());
        env.set_field(&jshare, "sharedBy", "Ljava/lang/String;", JValue::Object(&shared_by))
            .unwrap();

        // shared with
        let shared_with = jni_string(env, share.shared_with.clone());
        env.set_field(&jshare, "sharedWith", "Ljava/lang/String;", JValue::Object(&shared_with))
            .unwrap();
        env.set_object_array_element(&shares_array, i as i32, jshare)
            .unwrap();
    }

    env.set_field(&obj, "shares", "[Lnet/lockbook/File$Share;", JValue::Object(&shares_array))
        .unwrap();

    obj
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getRoot<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    match lb.get_root() {
        Ok(file) => jfile(&mut env, file),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

fn jfiles<'local>(env: &mut JNIEnv<'local>, rust_files: Vec<File>) -> JObjectArray<'local> {
    let file_class = env.find_class("Lnet/lockbook/File;").unwrap();
    let obj = env
        .new_object_array(rust_files.len() as i32, file_class, JObject::null())
        .unwrap();

    for (i, rust_file) in rust_files.iter().enumerate() {
        let file = jfile(env, rust_file.clone());
        env.set_object_array_element(&obj, i as i32, file).unwrap();
    }

    obj
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getChildren<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, id: JString<'local>,
) -> jobjectArray {
    let lb = rlb(&mut env, &class);
    let id = Uuid::from_str(&rstring(&mut env, id)).unwrap();

    match lb.get_children(&id) {
        Ok(files) => jfiles(&mut env, files).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getFileById<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    match lb.get_file_by_id(id) {
        Ok(file) => jfile(&mut env, file),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_renameFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>, jname: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let name = rstring(&mut env, jname);

    if let Err(err) = lb.rename_file(&id, &name) {
        throw_err(&mut env, err);
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_createFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jname: JString<'local>,
    jparent_id: JString<'local>, jis_doc: jboolean,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let name = rstring(&mut env, jname);
    let parent_id = Uuid::from_str(&rstring(&mut env, jparent_id)).unwrap();
    let file_type = if jis_doc == 1 { FileType::Document } else { FileType::Folder };

    match lb.create_file(&name, &parent_id, file_type) {
        Ok(file) => jfile(&mut env, file),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_createLink<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jname: JString<'local>,
    jtarget_id: JString<'local>, jparent_id: JString<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let name = rstring(&mut env, jname);
    let target = Uuid::from_str(&rstring(&mut env, jtarget_id)).unwrap();
    let parent_id = Uuid::from_str(&rstring(&mut env, jparent_id)).unwrap();
    let file_type = FileType::Link { target };

    match lb.create_file(&name, &parent_id, file_type) {
        Ok(file) => jfile(&mut env, file),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_convertToHumanDuration(
    mut env: JNIEnv, _: JClass, time_stamp: jlong,
) -> jstring {
    let msg = if time_stamp != 0 {
        Duration::milliseconds(clock::get_time().0 - time_stamp)
            .format_human()
            .to_string()
    } else {
        "never".to_string()
    };

    jni_string(&mut env, msg).into_raw()
}

fn jusage_item_metric<'local>(env: &mut JNIEnv<'local>, usage: UsageItemMetric) -> JObject<'local> {
    let item_metric_class = env
        .find_class("Lnet/lockbook/Usage$UsageItemMetric;")
        .unwrap();
    let obj = env.alloc_object(item_metric_class).unwrap();

    env.set_field(&obj, "exact", "J", JValue::Long(usage.exact as i64))
        .unwrap();

    let readable = jni_string(env, usage.readable);
    env.set_field(&obj, "readable", "Ljava/lang/String;", JValue::Object(&readable))
        .unwrap();

    obj
}

fn jusage_metrics<'local>(env: &mut JNIEnv<'local>, usage: UsageMetrics) -> JObject<'local> {
    let usage_class = env
        .find_class("Lnet/lockbook/Usage$UsageItemMetric;")
        .unwrap();
    let obj = env.alloc_object(usage_class).unwrap();

    let server_usage = jusage_item_metric(env, usage.server_usage);
    env.set_field(
        &obj,
        "serverUsage",
        "Lnet/lockbook/File$ShareMode;",
        JValue::Object(&server_usage),
    )
    .unwrap();

    let data_cap = jusage_item_metric(env, usage.data_cap);
    env.set_field(&obj, "dataCap", "Lnet/lockbook/File$ShareMode;", JValue::Object(&data_cap))
        .unwrap();

    obj
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getUsage<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.get_usage() {
        Ok(usage) => jusage_metrics(&mut env, usage),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getUncompressedUsage<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.get_uncompressed_usage() {
        Ok(usage) => jusage_item_metric(&mut env, usage),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_deleteFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    if let Err(err) = lb.delete_file(&id) {
        throw_err(&mut env, err);
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_readDocument<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    match lb.read_document(id) {
        Ok(doc) => jni_string(&mut env, String::from(String::from_utf8_lossy(&doc))).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_readDocumentBytes<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) -> jbyteArray {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    match lb.read_document(id) {
        Ok(doc) => jbyte_array(&mut env, doc).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_writeDocument<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>, jcontent: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let content = rstring(&mut env, jcontent);

    if let Err(err) = lb.write_document(id, &content.as_bytes()) {
        throw_err(&mut env, err);
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_writeDocumentBytes<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
    jcontent: JByteArray<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let content = rbyte_array(&mut env, jcontent);

    if let Err(err) = lb.write_document(id, &content) {
        throw_err(&mut env, err);
    }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_moveFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
    jparent_id: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let parent_id = Uuid::from_str(&rstring(&mut env, jparent_id)).unwrap();

    if let Err(err) = lb.move_file(&id, &parent_id) {
        throw_err(&mut env, err);
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    mut env: JNIEnv<'static>, class: JClass, jsync_progress: JObject<'static>,
) {
    // let lb: &mut Lb = rlb(&mut env, &class);

    // let env_c = env.unsafe_clone();
    // let closure = move |sync_progress: SyncProgress| {
    //     let msg = jni_string(&mut env_c, sync_progress.msg);
    //     let args = [
    //         JValue::Int(sync_progress.total as i32),
    //         JValue::Int(sync_progress.progress as i32),
    //         JValue::Object(&msg),
    //     ]
    //     .to_vec();

    //     env_c
    //         .call_method(
    //             jsync_progress,
    //             "updateSyncProgressAndTotal",
    //             "(IILjava/lang/String;)V",
    //             args.as_slice(),
    //         )
    //         .unwrap();
    // };

    // if let Err(err) = lb.sync(Some(Box::new(closure))) {
    //     throw_err(&mut env, err);
    // }
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_backgroundSync(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_calculateWork(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_exportFile(
    env: JNIEnv, _: JClass, jid: JString, jdestination: JString, jedit: jboolean,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_upgradeAccountGooglePlay(
    env: JNIEnv, _: JClass, jpurchase_token: JString, jaccount_id: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_cancelSubscription(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getSubscriptionInfo(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getLocalChanges(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_listMetadatas(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_search(
    env: JNIEnv, _: JClass, jquery: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_shareFile(
    env: JNIEnv, _: JClass, jid: JString, jusername: JString, jmode: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getPendingShares(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_deletePendingShare(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_suggestedDocs(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_logout(_env: JNIEnv, _: JClass) {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_deleteAccount(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}
