use jni::JNIEnv;
use jni::objects::{JClass, JString};

use crate::{init_logger_safely, create_account, import_account, get_root, get_children, get_file_by_id, insert_file, delete_file, create_file, write_document, read_document, rename_file, DB_NAME};
use crate::model::state::Config;
use uuid::Uuid;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::crypto::DecryptedValue;
use jni::sys::jboolean;
use std::path::Path;
use serde::Serialize;

fn serialize_to_jstring< U: Serialize>(env: JNIEnv, result: U) -> JString {
    let serialized_result = serde_json::to_string(&result).expect("Couldn't serialize result into result string!");
    env.new_string(serialized_result).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_initLogger(
    _env: JNIEnv,
    _: JClass,
) {
    init_logger_safely()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_isDbPresent(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
) -> jboolean {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let db_path = path + "/" + DB_NAME;
    debug!("Checking if {:?} exists", db_path);
    if Path::new(db_path.as_str()).exists() {
        debug!("DB Exists!");
        1
    } else {
        error!("DB Does not exist!");
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jusername: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let username: String = env
        .get_string(jusername)
        .expect("Couldn't read the username out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config (path) string into config!");

    serialize_to_jstring(env, create_account(&deserialized_config, username.as_str()))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_Corekt_importAccount<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jaccount: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_account: String = env
        .get_string(jaccount)
        .expect("Couldn't read the account string out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    serialize_to_jstring(env, import_account(&deserialized_config, serialized_account.as_str()))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    serialize_to_jstring(env, get_root(&deserialized_config))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_Corekt_getChildren<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, get_children(&deserialized_config, deserialized_id))
}

pub extern "system" fn Java_app_lockbook_core_Corekt_getFileById<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, get_file_by_id(&deserialized_config, deserialized_id))
}

pub extern "system" fn Java_app_lockbook_core_Corekt_insertFile<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jfilemetadata: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_file_metadata: String = env
        .get_string(jfilemetadata)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_file_metadata: FileMetadata = serde_json::from_str(&serialized_file_metadata).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, insert_file(&deserialized_config, deserialized_file_metadata))
}

pub extern "system" fn Java_app_lockbook_core_Corekt_deleteFile<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, delete_file(&deserialized_config, deserialized_id))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createFile<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jname: JString,
    jid: JString,
    jfiletype: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let name: String = env
        .get_string(jname)
        .expect("Couldn't read the name out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let serialized_filetype: String = env
        .get_string(jfiletype)
        .expect("Couldn't read the filetype out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    let deserialized_filetype: FileType = serde_json::from_str(&serialized_filetype).expect("Couldn't deserialize filetype string into filetype!");

    serialize_to_jstring(env, create_file(&deserialized_config, name.as_str(), deserialized_id, deserialized_filetype))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_Corekt_writeDocument<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jcontent: JString
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let serialized_content: String = env
        .get_string(jcontent)
        .expect("Couldn't read the content out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    let deserialized_content: DecryptedValue = serde_json::from_str(&serialized_content).expect("Couldn't deserialize content string into content!");

    serialize_to_jstring(env, write_document(&deserialized_config, deserialized_id, &deserialized_content))
}

pub extern "system" fn Java_app_lockbook_core_Corekt_readDocument<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, read_document(&deserialized_config, deserialized_id))
}

pub extern "system" fn Java_app_lockbook_core_Corekt_renameFile<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jname: JString
) -> JString<'a> {
    let serialized_config: String = env
        .get_string(jconfig)
        .expect("Couldn't read the config (path) out of JNI!")
        .into();

    let serialized_id: String = env
        .get_string(jid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let name: String = env
        .get_string(jname)
        .expect("Couldn't read the name out of JNI!")
        .into();

    let deserialized_config: Config = serde_json::from_str(&serialized_config).expect("Couldn't deserialize config string (path) into config!");

    let deserialized_id: Uuid = Uuid::parse_str(&serialized_id).expect("Couldn't deserialize id string into id!");

    serialize_to_jstring(env, rename_file(&deserialized_config, deserialized_id, name.as_str()))
}


