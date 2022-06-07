#![allow(non_snake_case)]

use std::path::PathBuf;
use std::str::FromStr;
use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jarray, jboolean, jbyteArray, jintArray, jlong, jsize, jstring};
use jni::JNIEnv;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    get_all_error_variants, unexpected_only, Config, Error, SupportedImageFormats, UnexpectedError,
};
use lockbook_crypto::clock_service;
use lockbook_models::file_metadata::FileType;
use lockbook_models::work_unit::ClientWorkUnit;

use crate::external_interface::json_interface::translate;
use crate::external_interface::static_state;
use crate::service::sync_service::SyncProgress;

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
            env,
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
            env,
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
            translate::<(), UnexpectedError>(Err(unexpected_only!(
                "Couldn't deserialize {}: {:?}",
                name,
                err
            ))),
        )
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_init(
    env: JNIEnv, _: JClass, jconfig: JString,
) -> jstring {
    let config = match deserialize::<Config>(&env, jconfig, "config") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(static_state::init(&config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount(
    env: JNIEnv, _: JClass, jusername: JString, japi_url: JString,
) -> jstring {
    let username = match jstring_to_string(&env, jusername, "username") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let api_url = match jstring_to_string(&env, japi_url, "api_url") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.create_account(&username, &api_url)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_importAccount(
    env: JNIEnv, _: JClass, jaccount: JString,
) -> jstring {
    let account = match jstring_to_string(&env, jaccount, "account") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.import_account(account.as_str())),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportAccount(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.export_account()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAccount(env: JNIEnv, _: JClass) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_account()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot(env: JNIEnv, _: JClass) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_root()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getChildren(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(&env, translate(static_state::get().and_then(|core| core.get_children(id))))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFileById(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_file_by_id(id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFile(
    env: JNIEnv, _: JClass, jid: JString, jname: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let name = &match jstring_to_string(&env, jname, "name") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.rename_file(id, name)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createFile(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jfiletype: JString,
) -> jstring {
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
        match static_state::get() {
            Ok(core) => translate(core.create_file(name.as_str(), id, file_type)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_convertToHumanDuration(
    env: JNIEnv, _: JClass, metadata_version: jlong,
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
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUsage(env: JNIEnv, _: JClass) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_usage()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUncompressedUsage(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_uncompressed_usage()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFile(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.delete_file(id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocument(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.read_document(id).map(|b| String::from(String::from_utf8_lossy(&b)))),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocumentBytes(
    env: JNIEnv, _: JClass, jid: JString,
) -> jbyteArray {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let mut document_bytes = static_state::get().unwrap().read_document(id).ok().unwrap();

    env.byte_array_from_slice(document_bytes.as_slice()).unwrap()

}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_saveDocumentToDisk(
    env: JNIEnv, _: JClass, jid: JString, jlocation: JString,
) -> jstring {
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
        match static_state::get() {
            Ok(core) => translate(core.save_document_to_disk(id, &location)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportDrawingToDisk(
    env: JNIEnv, _: JClass, jid: JString, jformat: JString, jlocation: JString,
) -> jstring {
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
        match static_state::get() {
            Ok(core) => translate(core.export_drawing_to_disk(id, format, None, &location)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_writeDocument(
    env: JNIEnv, _: JClass, jid: JString, jcontent: JString,
) -> jstring {
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
        match static_state::get() {
            Ok(core) => translate(core.write_document(id, &content.into_bytes())),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_moveFile(
    env: JNIEnv, _: JClass, jid: JString, jparentid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let parent_id = match deserialize_id(&env, jparentid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.move_file(id, parent_id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    env: JNIEnv<'static>, _: JClass, jsyncmodel: JObject<'static>,
) -> jstring {
    let env_c = env.clone();
    let closure = move |sync_progress: SyncProgress| {
        let (is_pushing, file_name) = match sync_progress.current_work_unit {
            ClientWorkUnit::PullMetadata => (JValue::Bool(0), JValue::Object(JObject::null())),
            ClientWorkUnit::PushMetadata => (JValue::Bool(1), JValue::Object(JObject::null())),
            ClientWorkUnit::PullDocument(file_name) => {
                let obj = env_c
                    .new_string(file_name)
                    .expect("Couldn't create JString from rust string!");

                (JValue::Bool(0), JValue::Object(JObject::from(obj)))
            }
            ClientWorkUnit::PushDocument(file_name) => {
                let obj = env_c
                    .new_string(file_name)
                    .expect("Couldn't create JString from rust string!");

                (JValue::Bool(1), JValue::Object(JObject::from(obj)))
            }
        };

        let args = [
            JValue::Int(sync_progress.total as i32),
            JValue::Int(sync_progress.progress as i32),
            is_pushing,
            file_name,
        ]
        .to_vec();
        env_c
            .call_method(
                jsyncmodel,
                "updateSyncProgressAndTotal",
                "(IIZLjava/lang/String;)V",
                args.as_slice(),
            )
            .unwrap();
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.sync(Some(Box::new(closure)))),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_backgroundSync(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.sync(None)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_calculateWork(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.calculate_work()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportFile(
    env: JNIEnv, _: JClass, jid: JString, jdestination: JString, jedit: jboolean
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let destination = match jstring_to_string(&env, jdestination, "path").and_then(|destination_str| {
        PathBuf::from_str(&destination_str).map_err(|_| string_to_jstring(&env, "Could not parse destination as PathBuf.".to_string()))
    }) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    println!("YO WHAT THE {}", jedit);
    let edit = jedit == 1;

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.export_file(id, destination, edit, None)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAllErrorVariants(
    env: JNIEnv, _: JClass,
) -> jstring {
    serialize_to_jstring(&env, get_all_error_variants())
}
