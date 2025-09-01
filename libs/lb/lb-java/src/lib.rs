mod java_utils;

use std::fs;
use std::str::FromStr;

use java_utils::{jbyte_array, jni_string, rbyte_array, rlb, rstring, throw_err};
use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue};
use jni::sys::{jboolean, jbyteArray, jlong, jobject, jobjectArray, jstring};
pub use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::api::{
    AppStoreAccountState, GooglePlayAccountState, PaymentPlatform, SubscriptionInfo,
};
pub use lb_rs::model::core_config::Config;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::work_unit::WorkUnit;
use lb_rs::service::activity::RankingWeights;
use lb_rs::service::sync::{SyncProgress, SyncStatus};
use lb_rs::service::usage::{UsageItemMetric, UsageMetrics};
pub use lb_rs::*;
use subscribers::search::{SearchConfig, SearchResult};

use crate::java_utils::rpaths_array;

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_init<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, path: JString<'local>,
) {
    let config = Config {
        writeable_path: rstring(&mut env, path),
        background_work: true,
        logs: true,
        stdout_logs: true,
        colored_logs: false,
    };

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

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getDebugInfo<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, os_info: JString<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    let os_info = rstring(&mut env, os_info);
    jni_string(&mut env, lb.debug_info(os_info)).into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_createAccount<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, username: JString<'local>,
    api_url: JString<'local>, welcome_doc: jboolean,
) -> jobject {
    let lb = rlb(&mut env, &class);

    let username = rstring(&mut env, username);
    let api_url = if api_url.is_null() {
        DEFAULT_API_LOCATION.to_string()
    } else {
        rstring(&mut env, api_url)
    };
    let welcome_doc = welcome_doc != 0;

    match lb.create_account(&username, &api_url, welcome_doc) {
        Ok(account) => j_account(&mut env, account),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[unsafe(no_mangle)]
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
    let obj = env.find_class("net/lockbook/Account").unwrap();
    let obj = env.alloc_object(obj).unwrap();

    let username = jni_string(env, account.username);
    let api_url = jni_string(env, account.api_url);

    env.set_field(&obj, "username", "Ljava/lang/String;", JValue::Object(&username))
        .unwrap();
    env.set_field(&obj, "apiUrl", "Ljava/lang/String;", JValue::Object(&api_url))
        .unwrap();

    obj
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_exportAccountPrivateKey<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.export_account_private_key() {
        Ok(account) => jni_string(&mut env, account).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_exportAccountPhrase<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    match lb.export_account_phrase() {
        Ok(account) => jni_string(&mut env, account).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
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
    let file_class = env.find_class("net/lockbook/File").unwrap();
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
    let enum_class = env.find_class("net/lockbook/File$FileType").unwrap();
    let filetype_name = match file.file_type {
        FileType::Document => "Document",
        FileType::Folder => "Folder",
        FileType::Link { .. } => "Link",
    };
    let enum_constant = env
        .get_static_field(enum_class, filetype_name, "Lnet/lockbook/File$FileType;")
        .unwrap()
        .l()
        .unwrap();

    env.set_field(&obj, "type", "Lnet/lockbook/File$FileType;", JValue::Object(&enum_constant))
        .unwrap();

    // last modified
    env.set_field(&obj, "lastModified", "J", JValue::Long(file.last_modified as jlong))
        .unwrap();

    // last modified by
    let last_modified_by = jni_string(env, file.last_modified_by);
    env.set_field(&obj, "lastModifiedBy", "Ljava/lang/String;", JValue::Object(&last_modified_by))
        .unwrap();

    let share_class = env.find_class("net/lockbook/File$Share").unwrap();
    let share_mode_class = env.find_class("net/lockbook/File$ShareMode").unwrap();

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

#[unsafe(no_mangle)]
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

fn jfiles<'local>(env: &mut JNIEnv<'local>, files: Vec<File>) -> JObjectArray<'local> {
    let file_class = env.find_class("net/lockbook/File").unwrap();
    let obj = env
        .new_object_array(files.len() as i32, file_class, JObject::null())
        .unwrap();

    for (i, rust_file) in files.iter().enumerate() {
        let file = jfile(env, rust_file.clone());
        env.set_object_array_element(&obj, i as i32, file).unwrap();
    }

    obj
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getTimestampHumanString<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, timestamp: jlong,
) -> jstring {
    let lb = rlb(&mut env, &class);

    jni_string(&mut env, lb.get_timestamp_human_string(timestamp)).into_raw()
}

fn jusage_item_metric<'local>(env: &mut JNIEnv<'local>, usage: UsageItemMetric) -> JObject<'local> {
    let item_metric_class = env
        .find_class("net/lockbook/Usage$UsageItemMetric")
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
    let usage_class = env.find_class("net/lockbook/Usage").unwrap();
    let obj = env.alloc_object(usage_class).unwrap();

    let server_usage = jusage_item_metric(env, usage.server_usage);
    env.set_field(
        &obj,
        "serverUsage",
        "Lnet/lockbook/Usage$UsageItemMetric;",
        JValue::Object(&server_usage),
    )
    .unwrap();

    let data_cap = jusage_item_metric(env, usage.data_cap);
    env.set_field(
        &obj,
        "dataCap",
        "Lnet/lockbook/Usage$UsageItemMetric;",
        JValue::Object(&data_cap),
    )
    .unwrap();

    obj
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getUsage<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    match lb.get_usage() {
        Ok(usage) => jusage_metrics(&mut env, usage),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_deleteFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    if let Err(err) = lb.delete_file(&id) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_readDocument<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    match lb.read_document(id, false) {
        Ok(doc) => jni_string(&mut env, String::from(String::from_utf8_lossy(&doc))).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_readDocumentBytes<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) -> jbyteArray {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    // todo: expose activity field when desired
    match lb.read_document(id, false) {
        Ok(doc) => jbyte_array(&mut env, doc).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_writeDocument<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>, jcontent: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let content = rstring(&mut env, jcontent);

    if let Err(err) = lb.write_document(id, content.as_bytes()) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_writeDocumentBytes<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
    jcontent: JByteArray<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let content = rbyte_array(&env, jcontent);

    if let Err(err) = lb.write_document(id, &content) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_sync<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jsync_progress: JObject<'local>,
) {
    let lb: &mut Lb = rlb(&mut env, &class);

    let f: Option<Box<dyn Fn(SyncProgress) + Send>> = if jsync_progress.is_null() {
        None
    } else {
        let jvm = env.get_java_vm().unwrap();
        let jsync_progress = env.new_global_ref(jsync_progress).unwrap();

        Some(Box::new(move |sync_progress: SyncProgress| {
            let mut env = jvm.attach_current_thread().unwrap();

            let msg = jni_string(&mut env, sync_progress.msg);
            let args = [
                JValue::Int(sync_progress.total as i32),
                JValue::Int(sync_progress.progress as i32),
                JValue::Object(&msg),
            ]
            .to_vec();

            env.call_method(
                jsync_progress.as_obj(),
                "updateSyncProgressAndTotal",
                "(IILjava/lang/String;)V",
                args.as_slice(),
            )
            .unwrap();
        }))
    };

    if let Err(err) = lb.sync(f) {
        throw_err(&mut env, err);
    }
}

fn jsync_status<'local>(env: &mut JNIEnv<'local>, sync_status: SyncStatus) -> JObject<'local> {
    let sync_status_class = env.find_class("net/lockbook/SyncStatus").unwrap();
    let sync_status_obj = env.alloc_object(sync_status_class).unwrap();

    // latest server ts
    env.set_field(
        &sync_status_obj,
        "latestServerTS",
        "J",
        JValue::Long(sync_status.latest_server_ts as jlong),
    )
    .unwrap();

    // work units
    let work_unit_class = env.find_class("net/lockbook/SyncStatus$WorkUnit").unwrap();

    let work_units_array = env
        .new_object_array(sync_status.work_units.len() as i32, &work_unit_class, JObject::null())
        .unwrap();

    for (i, work_unit) in sync_status.work_units.iter().enumerate() {
        let work_unit_obj = env.alloc_object(&work_unit_class).unwrap();

        let (id, is_local_change) = match work_unit {
            WorkUnit::LocalChange(id) => (id, 1),
            WorkUnit::ServerChange(id) => (id, 0),
        };

        // id
        let id = jni_string(env, id.to_string());
        env.set_field(&work_unit_obj, "id", "Ljava/lang/String;", JValue::Object(&id))
            .unwrap();

        // is local change
        env.set_field(&work_unit_obj, "isLocalChange", "Z", JValue::Bool(is_local_change))
            .unwrap();

        env.set_object_array_element(&work_units_array, i as i32, work_unit_obj)
            .unwrap();
    }

    env.set_field(
        &sync_status_obj,
        "workUnits",
        "[Lnet/lockbook/SyncStatus$WorkUnit;",
        JValue::Object(&work_units_array),
    )
    .unwrap();

    sync_status_obj
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_calculateWork<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobject {
    let lb: &mut Lb = rlb(&mut env, &class);

    match lb.calculate_work() {
        Ok(sync_status) => jsync_status(&mut env, sync_status),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_exportFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>, jdest: JString<'local>,
    jedit: jboolean,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let dest = rstring(&mut env, jdest).parse().unwrap();
    let edit = jedit == 1;

    if let Err(err) = lb.export_files(id, dest, edit, &None) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_importFiles<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, sources: JObjectArray<'local>,
    jdest: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let dest = Uuid::from_str(&rstring(&mut env, jdest)).unwrap();
    let sources = rpaths_array(&mut env, sources);

    if let Err(err) = lb.import_files(&sources, dest, &|_| {}) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_upgradeAccountGooglePlay<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jpurchase_token: JString<'local>,
    jaccount_id: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let purchase_token = rstring(&mut env, jpurchase_token);
    let account_id = rstring(&mut env, jaccount_id);

    if let Err(err) = lb.upgrade_account_google_play(&purchase_token, &account_id) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_cancelSubscription<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) {
    let lb = rlb(&mut env, &class);

    if let Err(err) = lb.cancel_subscription() {
        throw_err(&mut env, err);
    }
}

fn jsubscription_info<'local>(
    env: &mut JNIEnv<'local>, sub_info: Option<SubscriptionInfo>,
) -> JObject<'local> {
    let sub_info = match sub_info {
        Some(sub_info) => sub_info,
        None => return JObject::null(),
    };

    let subscription_info_class = env.find_class("net/lockbook/SubscriptionInfo").unwrap();
    let obj = env.alloc_object(subscription_info_class).unwrap();

    env.set_field(&obj, "periodEnd", "J", JValue::Long(sub_info.period_end as jlong))
        .unwrap();

    match sub_info.payment_platform {
        PaymentPlatform::Stripe { card_last_4_digits } => {
            let stripe_class = env
                .find_class("net/lockbook/SubscriptionInfo$Stripe")
                .unwrap();
            let stripe_obj = env.alloc_object(stripe_class).unwrap();

            let card_last_4_digits = jni_string(env, card_last_4_digits);
            env.set_field(
                &stripe_obj,
                "cardLast4Digits",
                "Ljava/lang/String;",
                JValue::Object(&card_last_4_digits),
            )
            .unwrap();

            env.set_field(
                &obj,
                "paymentPlatform",
                "Lnet/lockbook/SubscriptionInfo$PaymentPlatform;",
                JValue::Object(&stripe_obj),
            )
            .unwrap();
        }
        PaymentPlatform::GooglePlay { account_state } => {
            let google_play_class = env
                .find_class("net/lockbook/SubscriptionInfo$GooglePlay")
                .unwrap();
            let google_play_obj = env.alloc_object(google_play_class).unwrap();

            let google_play_enum_class = env
                .find_class("net/lockbook/SubscriptionInfo$GooglePlay$GooglePlayAccountState")
                .unwrap();
            let account_state_str = match account_state {
                GooglePlayAccountState::Ok => "Ok",
                GooglePlayAccountState::Canceled => "Canceled",
                GooglePlayAccountState::GracePeriod => "GracePeriod",
                GooglePlayAccountState::OnHold => "OnHold",
            };
            let account_state_enum = env
                .get_static_field(
                    google_play_enum_class,
                    account_state_str,
                    "Lnet/lockbook/SubscriptionInfo$GooglePlay$GooglePlayAccountState;",
                )
                .unwrap()
                .l()
                .unwrap();

            env.set_field(
                &google_play_obj,
                "accountState",
                "Lnet/lockbook/SubscriptionInfo$GooglePlay$GooglePlayAccountState;",
                JValue::Object(&account_state_enum),
            )
            .unwrap();

            env.set_field(
                &obj,
                "paymentPlatform",
                "Lnet/lockbook/SubscriptionInfo$PaymentPlatform;",
                JValue::Object(&google_play_obj),
            )
            .unwrap();
        }
        PaymentPlatform::AppStore { account_state } => {
            let app_store_class = env
                .find_class("net/lockbook/SubscriptionInfo$AppStore")
                .unwrap();
            let app_store_obj = env.alloc_object(app_store_class).unwrap();

            let app_store_enum_class = env
                .find_class("net/lockbook/SubscriptionInfo$AppStore$AppStoreAccountState")
                .unwrap();
            let account_state_str = match account_state {
                AppStoreAccountState::Ok => "Ok",
                AppStoreAccountState::GracePeriod => "GracePeriod",
                AppStoreAccountState::FailedToRenew => "FailedToRenew",
                AppStoreAccountState::Expired => "Expired",
            };
            let account_state_enum = env
                .get_static_field(
                    app_store_enum_class,
                    account_state_str,
                    "Lnet/lockbook/SubscriptionInfo$AppStore$AppStoreAccountState;",
                )
                .unwrap()
                .l()
                .unwrap();

            env.set_field(
                &app_store_obj,
                "accountState",
                "Lnet/lockbook/SubscriptionInfo$AppStore$AppStoreAccountState;",
                JValue::Object(&account_state_enum),
            )
            .unwrap();

            env.set_field(
                &obj,
                "paymentPlatform",
                "Lnet/lockbook/SubscriptionInfo$PaymentPlatform;",
                JValue::Object(&app_store_obj),
            )
            .unwrap();
        }
    }

    obj
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getSubscriptionInfo<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobject {
    let lb = rlb(&mut env, &class);

    match lb.get_subscription_info() {
        Ok(sub_info) => jsubscription_info(&mut env, sub_info),
        Err(err) => throw_err(&mut env, err),
    }
    .into_raw()
}

fn jids<'local>(env: &mut JNIEnv<'local>, ids: Vec<Uuid>) -> JObjectArray<'local> {
    let string_class = env.find_class("java/lang/String").unwrap();

    let arr = env
        .new_object_array(ids.len() as i32, string_class, JObject::null())
        .unwrap();

    for (i, id) in ids.iter().enumerate() {
        let id = env.new_string(id.to_string()).unwrap();
        env.set_object_array_element(&arr, i as i32, JObject::from(id))
            .unwrap();
    }

    arr
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getLocalChanges<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobjectArray {
    let lb = rlb(&mut env, &class);

    match lb.get_local_changes() {
        Ok(local_changes) => jids(&mut env, local_changes).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_listMetadatas<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobjectArray {
    let lb = rlb(&mut env, &class);

    match lb.list_metadatas() {
        Ok(files) => jfiles(&mut env, files).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

fn jsearch_results<'local>(
    env: &mut JNIEnv<'local>, search_results: Vec<SearchResult>,
) -> JObjectArray<'local> {
    let search_result_class = env.find_class("net/lockbook/SearchResult").unwrap();
    let document_match_class = env
        .find_class("net/lockbook/SearchResult$DocumentMatch")
        .unwrap();
    let path_match_class = env
        .find_class("net/lockbook/SearchResult$PathMatch")
        .unwrap();
    let content_match_class = env
        .find_class("net/lockbook/SearchResult$DocumentMatch$ContentMatch")
        .unwrap();

    let arr = env
        .new_object_array(search_results.len() as i32, search_result_class, JObject::null())
        .unwrap();

    for (i, search_result) in search_results.iter().enumerate() {
        let obj = match search_result {
            SearchResult::DocumentMatch { id, path, content_matches } => {
                let jdoc_match = env.alloc_object(&document_match_class).unwrap();

                // id
                let jid = env.new_string(id.to_string()).unwrap();
                env.set_field(&jdoc_match, "id", "Ljava/lang/String;", JValue::Object(&jid))
                    .unwrap();

                // path
                let jpath = env.new_string(path.clone()).unwrap();
                env.set_field(&jdoc_match, "path", "Ljava/lang/String;", JValue::Object(&jpath))
                    .unwrap();

                // content matches
                let jcontent_matches = env
                    .new_object_array(
                        content_matches.len() as i32,
                        &content_match_class,
                        JObject::null(),
                    )
                    .unwrap();

                for (j, content_match) in content_matches.iter().enumerate() {
                    let jcontent_match = env.alloc_object(&content_match_class).unwrap();

                    // paragraph
                    let jparagraph = env.new_string(content_match.paragraph.clone()).unwrap();
                    env.set_field(
                        &jcontent_match,
                        "paragraph",
                        "Ljava/lang/String;",
                        JValue::Object(&jparagraph),
                    )
                    .unwrap();

                    // matched indices
                    let jmatched_indices = env
                        .new_int_array(content_match.matched_indices.len() as i32)
                        .unwrap();
                    let matched_indices: Vec<i32> = content_match
                        .matched_indices
                        .iter()
                        .map(|&x| x as i32)
                        .collect();
                    env.set_int_array_region(&jmatched_indices, 0, &matched_indices)
                        .unwrap();
                    env.set_field(
                        &jcontent_match,
                        "matchedIndices",
                        "[I",
                        JValue::Object(&jmatched_indices),
                    )
                    .unwrap();

                    // score
                    env.set_field(
                        &jcontent_match,
                        "score",
                        "I",
                        JValue::Int(content_match.score as i32),
                    )
                    .unwrap();

                    env.set_object_array_element(&jcontent_matches, j as i32, jcontent_match)
                        .unwrap();
                }

                env.set_field(
                    &jdoc_match,
                    "contentMatches",
                    "[Lnet/lockbook/SearchResult$DocumentMatch$ContentMatch;",
                    JValue::Object(&jcontent_matches),
                )
                .unwrap();

                jdoc_match
            }
            SearchResult::PathMatch { id, path, matched_indices, score } => {
                let jpath_match = env.alloc_object(&path_match_class).unwrap();

                // id
                let jid = env.new_string(id.to_string()).unwrap();
                env.set_field(&jpath_match, "id", "Ljava/lang/String;", JValue::Object(&jid))
                    .unwrap();

                // path
                let jpath = env.new_string(path.clone()).unwrap();
                env.set_field(&jpath_match, "path", "Ljava/lang/String;", JValue::Object(&jpath))
                    .unwrap();

                // matched indices
                let jmatched_indices = env.new_int_array(matched_indices.len() as i32).unwrap();
                let matched_indices: Vec<i32> = matched_indices.iter().map(|&x| x as i32).collect();
                env.set_int_array_region(&jmatched_indices, 0, &matched_indices)
                    .unwrap();
                env.set_field(
                    &jpath_match,
                    "matchedIndices",
                    "[I",
                    JValue::Object(&jmatched_indices),
                )
                .unwrap();

                // score
                env.set_field(&jpath_match, "score", "I", JValue::Int(*score as i32))
                    .unwrap();

                jpath_match
            }
        };

        env.set_object_array_element(&arr, i as i32, obj).unwrap();
    }

    arr
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_search<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jinput: JString<'local>,
) -> jstring {
    let lb = rlb(&mut env, &class);
    let query = rstring(&mut env, jinput);

    match lb.search(&query, SearchConfig::PathsAndDocuments) {
        Ok(search_results) => jsearch_results(&mut env, search_results).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_shareFile<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
    jusername: JString<'local>, jis_write_mode: jboolean,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();
    let username = rstring(&mut env, jusername);
    let mode = if jis_write_mode == 1 { ShareMode::Write } else { ShareMode::Read };

    if let Err(err) = lb.share_file(id, &username, mode) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_getPendingShares<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobjectArray {
    let lb = rlb(&mut env, &class);

    match lb.get_pending_shares() {
        Ok(files) => jfiles(&mut env, files).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_deletePendingShare<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    if let Err(err) = lb.delete_pending_share(&id) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_suggestedDocs<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) -> jobjectArray {
    let lb = rlb(&mut env, &class);

    match lb.suggested_docs(RankingWeights::default()) {
        Ok(suggested_docs) => jids(&mut env, suggested_docs).into_raw(),
        Err(err) => throw_err(&mut env, err).into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_clearSuggested<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) {
    let lb = rlb(&mut env, &class);

    if let Err(err) = lb.clear_suggested() {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_clearSuggestedId<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let id = Uuid::from_str(&rstring(&mut env, jid)).unwrap();

    if let Err(err) = lb.clear_suggested_id(id) {
        throw_err(&mut env, err);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_logout<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) {
    let lb = rlb(&mut env, &class);
    fs::remove_dir_all(lb.get_config().writeable_path).unwrap(); // todo: deduplicate
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_net_lockbook_Lb_deleteAccount<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>,
) {
    let lb = rlb(&mut env, &class);

    if let Err(err) = lb.delete_account() {
        throw_err(&mut env, err);
    }
}
