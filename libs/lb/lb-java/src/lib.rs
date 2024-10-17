mod java_utils;

use java_utils::{populate_err, rstring};
use jni::{
    objects::{JClass, JObject, JString, JValue},
    sys::{jboolean, jbyteArray, jlong, jobject, jstring},
    JNIEnv,
};
use lb_rs::{blocking::Lb, model::core_config::Config};

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_init<'local>(
    mut env: JNIEnv<'local>, _class: JClass<'local>, input: JString<'local>,
) -> jobject {
    let writeable_path = rstring(&mut env, &input);
    let config = Config { logs: true, colored_logs: false, writeable_path };

    // InitRes res = new InitRes();
    let res = env
        .find_class("net/lockbook/InitRes")
        .expect("Class Not Found");
    let obj = env.alloc_object(res).unwrap();

    match Lb::init(config) {
        Ok(lb) => {
            let ptr = Box::into_raw(Box::new(lb)) as jlong;
            env.set_field(&obj, "lb", "J", JValue::Long(ptr)).unwrap();
        }
        Err(err) => {
            populate_err(&mut env, &obj, err);
        }
    };

    obj.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount(
    env: JNIEnv, _: JClass, jusername: JString, japi_url: JString, jwelcome_doc: jboolean,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_importAccount(
    env: JNIEnv, _: JClass, jaccount: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportAccount(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAccount(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getChildren(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFileById(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFile(
    env: JNIEnv, _: JClass, jid: JString, jname: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createFile(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jfiletype: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createLink(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jparentId: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_convertToHumanDuration(
    env: JNIEnv, _: JClass, time_stamp: jlong,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUsage(env: JNIEnv, _: JClass) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUncompressedUsage(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFile(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocument(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

// Unlike readDocument, this function does not return any specific type  of error. Any error will result in this function returning null.
#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocumentBytes(
    env: JNIEnv, _: JClass, jid: JString,
) -> jbyteArray {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_writeDocument(
    env: JNIEnv, _: JClass, jid: JString, jcontent: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_moveFile(
    env: JNIEnv, _: JClass, jid: JString, jparentid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    env: JNIEnv<'static>, _: JClass, jsyncmodel: JObject<'static>,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_backgroundSync(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_calculateWork(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportFile(
    env: JNIEnv, _: JClass, jid: JString, jdestination: JString, jedit: jboolean,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_upgradeAccountGooglePlay(
    env: JNIEnv, _: JClass, jpurchase_token: JString, jaccount_id: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_cancelSubscription(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getSubscriptionInfo(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getLocalChanges(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_listMetadatas(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_search(
    env: JNIEnv, _: JClass, jquery: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_shareFile(
    env: JNIEnv, _: JClass, jid: JString, jusername: JString, jmode: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getPendingShares(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deletePendingShare(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_suggestedDocs(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_logout(_env: JNIEnv, _: JClass) {
    todo!()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteAccount(
    env: JNIEnv, _: JClass,
) -> jstring {
    todo!()
}
