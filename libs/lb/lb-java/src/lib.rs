mod java_utils;

use std::str::FromStr;

use java_utils::{jbyte_array, jfile, jfiles, jni_string, rlb, rstring, throw_err};
use jni::{
    objects::{JClass, JObject, JString, JValue},
    sys::{jboolean, jbyteArray, jlong, jobject, jobjectArray, jstring},
    JNIEnv,
};
use lb_rs::{
    blocking::Lb, model::{account::Account, core_config::Config}, Uuid, DEFAULT_API_LOCATION
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

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getChildren<'local>(
    mut env: JNIEnv<'local>, class: JClass<'local>, id: JString<'local>
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
    mut env: JNIEnv<'local>, class: JClass<'local>, jid: JString<'local>
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
    mut env: JNIEnv<'local>, class: JClass<'local>, jname: JString<'local>, jparent: JString<'local>, jfiletype: JString<'local>,
) {
    let lb = rlb(&mut env, &class);

    let parent = Uuid::from_str(&rstring(&mut env, jparent)).unwrap();
    let name = rstring(&mut env, jname);
    
    if let Err(err) = lb.create_file(&id, &name) {
        throw_err(&mut env, err);
    }
}


#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_createFile(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jfiletype: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_createLink(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jparentId: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_convertToHumanDuration(
    env: JNIEnv, _: JClass, time_stamp: jlong,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getUsage(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_getUncompressedUsage(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_deleteFile(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_readDocument(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

// Unlike readDocument, this function does not return any specific type  of error. Any error will result in this function returning null.
#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_readDocumentBytes(
    env: JNIEnv, _: JClass, jid: JString,
) -> jbyteArray {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_writeDocument(
    env: JNIEnv, _: JClass, jid: JString, jcontent: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_moveFile(
    env: JNIEnv, _: JClass, jid: JString, jparentid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_syncAll(
    env: JNIEnv<'static>, _: JClass, jsyncmodel: JObject<'static>,
) -> jstring {
    todo!()
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
