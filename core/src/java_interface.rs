#![allow(non_snake_case)]

use std::path::Path;

use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;
use serde::Serialize;
use uuid::Uuid;

use crate::json_interface::translate;
use crate::model::account::Account;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::{
    calculate_work, create_account, create_file, delete_file, execute_work, export_account,
    get_account, get_all_error_variants, get_children, get_db_state, get_file_by_id, get_root,
    get_usage, import_account, init_logger, insert_file, migrate_db, move_file, read_document,
    rename_file, set_last_synced, sync_all, write_document, Error,
};
use serde::de::DeserializeOwned;

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

fn jstring_to_string(env: &JNIEnv, json: JString, err_msg: &str) -> Result<String, jstring> {
    env.get_string(json)
        .and_then(|ok| Ok(ok.into()))
        .map_err(|err| {
            serialize_to_jstring(
                &env,
                Error::<()>::Unexpected(format!("{}:{:?}", err_msg, err)),
            )
        })
}

fn deserialize<U: DeserializeOwned>(
    env: &JNIEnv,
    json: JString,
    err_msg: &str,
) -> Result<U, jstring> {
    let json_string = jstring_to_string(env, json, err_msg)?;

    serde_json::from_str(&json_string).map_err(|err| {
        serialize_to_jstring(
            &env,
            Error::<()>::Unexpected(format!("{}:{:?}", err_msg, err)),
        )
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_initLogger(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
) -> jstring {
    let absolute_path = match jstring_to_string(&env, jpath, "Couldn't get path!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let path = Path::new(&absolute_path);

    string_to_jstring(&env, translate(init_logger(path)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUsage(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_usage(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getDBState(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let username = match jstring_to_string(&env, jusername, "Couldn't get username!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let api_url = match jstring_to_string(&env, japi_url, "Couldn't get api_url!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let account = match jstring_to_string(&env, jaccount, "Couldn't get account!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(get_file_by_id(&config, id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_insertFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jfilemetadata: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let file_metadata =
        match deserialize::<FileMetadata>(&env, jfilemetadata, "Couldn't get file metadata!") {
            Ok(ok) => ok,
            Err(err) => return err,
        };

    string_to_jstring(&env, translate(insert_file(&config, file_metadata)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jname: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let name = match jstring_to_string(&env, jname, "Couldn't get name!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let file_type = match deserialize::<FileType>(&env, jfiletype, "Couldn't get filetype!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let name = match jstring_to_string(&env, jname, "Couldn't get name!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(create_file(&config, name.as_str(), id, file_type)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        translate(read_document(&config, id).map(|d| String::from(String::from_utf8_lossy(&d)))),
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let content = match jstring_to_string(&env, jcontent, "Couldn't get content!") {
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
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let id = match deserialize::<Uuid>(&env, jid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let parent_id = match deserialize::<Uuid>(&env, jparentid, "Couldn't get id!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(move_file(&config, id, parent_id)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(sync_all(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_calculateSyncWork(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(calculate_work(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_executeSyncWork(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jaccount: JString,
    jworkunit: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "Couldn't get config!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let account = match deserialize::<Account>(&env, jaccount, "Couldn't get account!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let work_unit = match deserialize::<WorkUnit>(&env, jworkunit, "Couldn't get work unit!") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(execute_work(&config, &account, work_unit)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAllErrorVariants(
    env: JNIEnv,
    _: JClass,
) -> jstring {
    serialize_to_jstring(&env, get_all_error_variants())
}
