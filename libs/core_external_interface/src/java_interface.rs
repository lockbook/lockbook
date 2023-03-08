#![allow(non_snake_case)]

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use crossbeam::channel::Sender;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring};
use jni::JNIEnv;
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::{Arc, Mutex};

use lockbook_core::service::search_service::{SearchRequest, SearchResult};
use lockbook_core::{
    clock, unexpected_only, ClientWorkUnit, Config, Drawing, FileType, ShareMode,
    SupportedImageFormats, SyncProgress, UnexpectedError, Uuid,
};

use crate::errors::Error;
use crate::get_all_error_variants;
use crate::json_interface::translate;
use crate::static_state;

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
    env: JNIEnv, _: JClass, jusername: JString, japi_url: JString, jwelcome_doc: jboolean,
) -> jstring {
    let username = match jstring_to_string(&env, jusername, "username") {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let api_url = match jstring_to_string(&env, japi_url, "api_url") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let welcome_doc = jwelcome_doc == 1;
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.create_account(&username, &api_url, welcome_doc)),
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
pub extern "system" fn Java_app_lockbook_core_CoreKt_createLink(
    env: JNIEnv, _: JClass, jname: JString, jid: JString, jparentId: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };
    let parent = match deserialize_id(&env, jparentId) {
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
            Ok(core) => translate(
                core.create_file(name.as_str(), parent, FileType::Link { target: id })
                    .map(|_| ()),
            ),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_convertToHumanDuration(
    env: JNIEnv, _: JClass, time_stamp: jlong,
) -> jstring {
    string_to_jstring(
        &env,
        if time_stamp != 0 {
            Duration::milliseconds(clock::get_time().0 - time_stamp)
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
            Ok(core) => translate(
                core.read_document(id)
                    .map(|b| String::from(String::from_utf8_lossy(&b))),
            ),
            e => translate(e.map(|_| ())),
        },
    )
}

// Unlike readDocument, this function does not return any specific type  of error. Any error will result in this function returning null.
#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocumentBytes(
    env: JNIEnv, _: JClass, jid: JString,
) -> jbyteArray {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    match static_state::get()
        .ok()
        .and_then(|core| core.read_document(id).ok())
    {
        None => std::ptr::null_mut() as jbyteArray,
        Some(document_bytes) => env
            .byte_array_from_slice(document_bytes.as_slice())
            .unwrap_or(std::ptr::null_mut() as jbyteArray),
    }
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
            ClientWorkUnit::PullDocument(f) => {
                let obj = env_c
                    .new_string(f.name)
                    .expect("Couldn't create JString from rust string!");

                (JValue::Bool(0), JValue::Object(JObject::from(obj)))
            }
            ClientWorkUnit::PushDocument(f) => {
                let obj = env_c
                    .new_string(f.name)
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
    env: JNIEnv, _: JClass, jid: JString, jdestination: JString, jedit: jboolean,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let destination =
        match jstring_to_string(&env, jdestination, "path").and_then(|destination_str| {
            destination_str.parse().map_err(|_| {
                string_to_jstring(&env, "Could not parse destination as PathBuf.".to_string())
            })
        }) {
            Ok(ok) => ok,
            Err(err) => return err,
        };

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
pub extern "system" fn Java_app_lockbook_core_CoreKt_upgradeAccountGooglePlay(
    env: JNIEnv, _: JClass, jpurchase_token: JString, jaccount_id: JString,
) -> jstring {
    let purchase_token = &match jstring_to_string(&env, jpurchase_token, "purchase token") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let account_id = &match jstring_to_string(&env, jaccount_id, "account id") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.upgrade_account_google_play(purchase_token, account_id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_cancelSubscription(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.cancel_subscription()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getSubscriptionInfo(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_subscription_info()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getLocalChanges(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_local_changes()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_listMetadatas(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.list_metadatas()),
            e => translate(e.map(|_| ())),
        },
    )
}

lazy_static! {
    static ref MAYBE_SEARCH_TX: Arc<Mutex<Option<Sender<SearchRequest>>>> =
        Arc::new(Mutex::new(None));
}

fn send_search_request(env: JNIEnv, request: SearchRequest) -> jstring {
    let result = MAYBE_SEARCH_TX
        .lock()
        .map_err(|_| UnexpectedError::new("Could not get lock".to_string()))
        .and_then(|maybe_lock| {
            maybe_lock
                .clone()
                .ok_or_else(|| UnexpectedError::new("No search lock.".to_string()))
        })
        .and_then(|search_tx| search_tx.send(request).map_err(UnexpectedError::from));

    string_to_jstring(&env, translate(result))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_startSearch(
    env: JNIEnv, _: JClass, jsearchFilesViewModel: JObject<'static>,
) -> jstring {
    let (results_rx, search_tx) = match static_state::get().and_then(|core| core.start_search()) {
        Ok(search_info) => (search_info.results_rx, search_info.search_tx),
        Err(e) => return string_to_jstring(&env, translate(Err::<(), _>(e))),
    };

    match MAYBE_SEARCH_TX.lock() {
        Ok(mut lock) => *lock = Some(search_tx),
        Err(_) => {
            return string_to_jstring(&env, translate(Err::<(), _>("Cannot get search lock.")))
        }
    }

    while let Ok(results) = results_rx.recv() {
        match results {
            SearchResult::Error(e) => return string_to_jstring(&env, translate(Err::<(), _>(e))),
            SearchResult::FileNameMatch { id, path, matched_indices, score } => {
                let matched_indices_json = match serde_json::to_string(&matched_indices) {
                    Ok(json) => json,
                    Err(_) => return string_to_jstring(&env, "Failed to parse json.".to_string()),
                };

                let args = [
                    JValue::Object(JObject::from(string_to_jstring(&env, id.to_string()))),
                    JValue::Object(JObject::from(string_to_jstring(&env, path))),
                    JValue::Int(score as jint),
                    JValue::Object(JObject::from(string_to_jstring(&env, matched_indices_json))),
                ]
                .to_vec();

                env.call_method(
                    jsearchFilesViewModel,
                    "addFileNameSearchResult",
                    "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;)V",
                    args.as_slice(),
                )
                .unwrap();
            }
            SearchResult::FileContentMatches { id, path, content_matches } => {
                let content_matches_json = match serde_json::to_string(&content_matches) {
                    Ok(json) => json,
                    Err(_) => return string_to_jstring(&env, "Failed to parse json.".to_string()),
                };

                let args = [
                    JValue::Object(JObject::from(string_to_jstring(&env, id.to_string()))),
                    JValue::Object(JObject::from(string_to_jstring(&env, path))),
                    JValue::Object(JObject::from(string_to_jstring(&env, content_matches_json))),
                ]
                .to_vec();

                env.call_method(
                    jsearchFilesViewModel,
                    "addFileContentSearchResult",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
                    args.as_slice(),
                )
                .unwrap();
            }
            SearchResult::NoMatch => {
                env.call_method(jsearchFilesViewModel, "noMatch", "()V", &[])
                    .unwrap();
            }
        }
    }

    match MAYBE_SEARCH_TX.lock() {
        Ok(mut lock) => *lock = None,
        Err(_) => {
            return string_to_jstring(&env, translate(Err::<(), _>("Cannot get search lock.")))
        }
    }

    string_to_jstring(&env, translate(Ok::<_, ()>(())))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_search(
    env: JNIEnv, _: JClass, jquery: JString,
) -> jstring {
    let query = match jstring_to_string(&env, jquery, "query") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    send_search_request(env, SearchRequest::Search { input: query })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_stopCurrentSearch(
    env: JNIEnv, _: JClass,
) -> jstring {
    send_search_request(env, SearchRequest::StopCurrentSearch)
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_endSearch(env: JNIEnv, _: JClass) -> jstring {
    send_search_request(env, SearchRequest::EndSearch)
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_shareFile(
    env: JNIEnv, _: JClass, jid: JString, jusername: JString, jmode: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let username = match jstring_to_string(&env, jusername, "username") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let mode = match deserialize::<ShareMode>(&env, jmode, "share mode") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.share_file(id, &username, mode)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getPendingShares(
    env: JNIEnv, _: JClass,
) -> jstring {
    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_pending_shares()),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deletePendingShare(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.delete_pending_share(id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getDrawing(
    env: JNIEnv, _: JClass, jid: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.get_drawing(id)),
            e => translate(e.map(|_| ())),
        },
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_saveDrawing(
    env: JNIEnv, _: JClass, jid: JString, jdrawing: JString,
) -> jstring {
    let id = match deserialize_id(&env, jid) {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    let drawing = match deserialize::<Drawing>(&env, jdrawing, "drawing") {
        Ok(ok) => ok,
        Err(err) => return err,
    };

    string_to_jstring(
        &env,
        match static_state::get() {
            Ok(core) => translate(core.save_drawing(id, &drawing)),
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
