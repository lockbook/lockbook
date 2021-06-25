#![allow(non_snake_case)]

use std::path::Path;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;
use uuid::Uuid;

use crate::json_interface::translate;
use crate::model::state::Config;
use crate::service::drawing_service::SupportedImageFormats;
use crate::service::sync_service::SyncProgress;
use crate::service::usage_service::bytes_to_human;
use crate::{
    calculate_work, create_account, create_file, delete_file, export_account, export_drawing,
    export_drawing_to_disk, get_account, get_all_error_variants, get_children, get_db_state,
    get_file_by_id, get_root, get_usage, import_account, init_logger, migrate_db, move_file,
    read_document, rename_file, save_document_to_disk, set_last_synced, sync_all, write_document,
    Error,
};
use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use lockbook_crypto::clock_service;
use lockbook_models::file_metadata::FileType;
use serde::de::DeserializeOwned;
use serde::Serialize;

fn serialize_to_jstring<U: Serialize>(env: &JNIEnv, result: U) -> jstring {
    let serialized_result =
        serde_json::to_string(&result).expect("Couldn't serialize result into result string!");

    env.new_string(serialized_result)
        .expect("Couldn't create JString from rust string!")
        .into_inner()
}

fn string_to_jstring(env: &JNIEnv, result: String) -> jstring {
    env.new_string(result)
        .expect("Couldn't create JString from rust string!")
        .into_inner()
}

fn jstring_to_string(env: &JNIEnv, json: JString, name: &str) -> Result<String, jstring> {
    env.get_string(json).map(|ok| ok.into()).map_err(|err| {
        string_to_jstring(
            &env,
            translate::<(), Error<()>>(Err(Error::<()>::Unexpected(format!(
                "Could not parse {} jstring: {:?}",
                name, err
            )))),
        )
    })
}

fn deserialize_id(env: &JNIEnv, json: JString) -> Result<Uuid, jstring> {
    let json_string = jstring_to_string(env, json, "id")?;

    Uuid::parse_str(&json_string).map_err(|err| {
        string_to_jstring(
            &env,
            translate::<(), Error<()>>(Err(Error::<()>::Unexpected(format!(
                "Couldn't deserialize id: {:?}",
                err
            )))),
        )
    })
}

fn deserialize<U: DeserializeOwned>(env: &JNIEnv, json: JString, name: &str) -> Result<U, jstring> {
    let json_string = jstring_to_string(env, json, name)?;

    serde_json::from_str::<U>(&json_string).map_err(|err| {
        string_to_jstring(
            env,
            translate::<(), Error<()>>(Err(Error::<()>::Unexpected(format!(
                "Couldn't deserialize {}: {:?}",
                name, err
            )))),
        )
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_initLogger(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
) -> jstring {
    let absolute_path = match jstring_to_string(&env, jpath, "path") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let path = Path::new(&absolute_path);

    string_to_jstring(&env, translate(init_logger(path)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getDBState(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_db_state(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_migrateDB(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(migrate_db(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jusername: JString,
    japi_url: JString,
) -> jstring {
    let username = match jstring_to_string(&env, jusername, "username") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let api_url = match jstring_to_string(&env, japi_url, "api_url") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(create_account(&config, &username, &api_url)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_importAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jaccount: JString,
) -> jstring {
    let account = match jstring_to_string(&env, jaccount, "account") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(import_account(&config, account.as_str())))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(export_account(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_account(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_setLastSynced(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jlastsynced: jlong,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(set_last_synced(&config, jlastsynced as u64)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_root(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getChildren(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_children(&config, id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFileById(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_file_by_id(&config, id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jname: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let name = match jstring_to_string(&env, jname, "name") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(rename_file(&config, id, name.as_str())))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jname: JString,
    jid: JString,
    jfiletype: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let file_type = match deserialize::<FileType>(&env, jfiletype, "filetype") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let name = match jstring_to_string(&env, jname, "name") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(create_file(&config, name.as_str(), id, file_type)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_convertToHumanDuration(
    env: JNIEnv,
    _: JClass,
    metadata_version: jlong,
) -> jstring {
    string_to_jstring(
        &env,
        if metadata_version != 0 {
            Duration::milliseconds(clock_service::get_time().0 - metadata_version)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUsage(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_usage(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_makeBytesReadable(
    env: JNIEnv,
    _: JClass,
    bytes: jlong,
) -> jstring {
    string_to_jstring(&env, bytes_to_human(bytes as u64).0)
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(delete_file(&config, id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocument(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(read_document(&config, id).map(|d| String::from(String::from_utf8_lossy(&d)))),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_saveDocumentToDisk(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jlocation: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let location = match jstring_to_string(&env, jlocation, "path") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(save_document_to_disk(&config, id, location)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportDrawing(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jformat: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let format = match deserialize::<SupportedImageFormats>(&env, jformat, "image format") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(export_drawing(&config, id, format, None)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportDrawingToDisk(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jformat: JString,
    jlocation: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let format = match deserialize::<SupportedImageFormats>(&env, jformat, "image format") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let location = match jstring_to_string(&env, jlocation, "path") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(export_drawing_to_disk(&config, id, format, None, location)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_writeDocument(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jcontent: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let content = match jstring_to_string(&env, jcontent, "content") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(write_document(&config, id, &content.into_bytes())),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_moveFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jparentid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let parent_id = match deserialize_id(&env, jparentid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(move_file(&config, id, parent_id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    env: JNIEnv<'static>,
    _: JClass,
    jconfig: JString,
    jsyncmodel: JObject<'static>,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let env_c = env.clone();
    let closure = move |sync_progress: SyncProgress| {
        let args = [
            JValue::Int(sync_progress.total as i32),
            JValue::Int(sync_progress.progress as i32),
        ]
        .to_vec();
        env_c
            .call_method(
                jsyncmodel,
                "updateSyncProgressAndTotal",
                "(II)V",
                args.as_slice(),
            )
            .unwrap();
    };

    string_to_jstring(&env, translate(sync_all(&config, Some(Box::new(closure)))))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_backgroundSync(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(sync_all(&config, None)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_calculateWork(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(calculate_work(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAllErrorVariants(
    env: JNIEnv,
    _: JClass,
) -> jstring {
    serialize_to_jstring(&env, get_all_error_variants())
}
