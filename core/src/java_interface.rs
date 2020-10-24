#![allow(non_snake_case)]

use std::path::Path;

use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;
use serde::Serialize;
use uuid::Uuid;

use crate::json_interface::translate;
use crate::model::account::Account;
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::{FileMetadata, FileType};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::{
    calculate_work, create_account, create_file, delete_file, execute_work, export_account,
    get_account, get_children, get_db_state, get_file_by_id, get_root, get_usage, import_account,
    init_logger, insert_file, migrate_db, move_file, read_document, rename_file, set_last_synced,
    sync_all, write_document, Error,
};

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

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_initLogger(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
) -> jstring {
    let absolute_path: String = match env.get_string(jpath) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get path out of JNI!".to_string()),
            );
        }
    }
    .into();

    let path = Path::new(&absolute_path);

    string_to_jstring(&env, translate(init_logger(path)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getUsage(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(get_usage(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getDBState(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(get_db_state(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_migrateDB(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(migrate_db(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jusername: JString,
    japi_url: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let username: String = match env.get_string(jusername) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get username out of JNI!".to_string()),
            );
        }
    }
    .into();

    let api_url: String = match env.get_string(japi_url) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get api_url out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(create_account(&deserialized_config, &username, &api_url)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_importAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jaccount: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_account: String = match env.get_string(jaccount) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get account out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(import_account(
            &deserialized_config,
            serialized_account.as_str(),
        )),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_exportAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(export_account(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getAccount(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(get_account(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_setLastSynced(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jlastsynced: jlong,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(set_last_synced(&deserialized_config, jlastsynced as u64)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(get_root(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getChildren(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(get_children(&deserialized_config, deserialized_id)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFileById(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(get_file_by_id(&deserialized_config, deserialized_id)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_insertFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jfilemetadata: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_file_metadata: String = match env.get_string(jfilemetadata) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get file metadata out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_file_metadata: FileMetadata =
        match serde_json::from_str(&serialized_file_metadata) {
            Ok(ok) => ok,
            Err(_) => {
                return serialize_to_jstring(
                    &env,
                    Error::<()>::Unexpected("Couldn't deserialize file metadata!".to_string()),
                );
            }
        };

    string_to_jstring(
        &env,
        translate(insert_file(
            &deserialized_config,
            deserialized_file_metadata,
        )),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
    jname: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let name: String = match env.get_string(jname) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get name out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(rename_file(
            &deserialized_config,
            deserialized_id,
            name.as_str(),
        )),
    )
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
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let name: String = match env.get_string(jname) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get name out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_filetype: String = match env.get_string(jfiletype) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get filetype out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    let deserialized_filetype: FileType = match serde_json::from_str(&serialized_filetype) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize filetype!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(create_file(
            &deserialized_config,
            name.as_str(),
            deserialized_id,
            deserialized_filetype,
        )),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFile(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(delete_file(&deserialized_config, deserialized_id)),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocument(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jid: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(read_document(&deserialized_config, deserialized_id)),
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
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_content: String = match env.get_string(jcontent) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get content out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    let deserialized_content: DecryptedValue = match serde_json::from_str(&serialized_content) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize content!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(write_document(
            &deserialized_config,
            deserialized_id,
            &deserialized_content,
        )),
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
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_id: String = match env.get_string(jid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_parent_id: String = match env.get_string(jparentid) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get id out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_id: Uuid = match Uuid::parse_str(&serialized_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    let deserialized_parent_id: Uuid = match Uuid::parse_str(&serialized_parent_id) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize id!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(move_file(
            &deserialized_config,
            deserialized_id,
            deserialized_parent_id,
        )),
    )
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_syncAll(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(sync_all(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_calculateSyncWork(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    string_to_jstring(&env, translate(calculate_work(&deserialized_config)))
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_executeSyncWork(
    env: JNIEnv,
    _: JClass,
    jconfig: JString,
    jaccount: JString,
    jworkunit: JString,
) -> jstring {
    let serialized_config: String = match env.get_string(jconfig) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get config out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_account: String = match env.get_string(jaccount) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get account out of JNI!".to_string()),
            );
        }
    }
    .into();

    let serialized_work_unit: String = match env.get_string(jworkunit) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't get work unit out of JNI!".to_string()),
            );
        }
    }
    .into();

    let deserialized_config: Config = match serde_json::from_str(&serialized_config) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize config!".to_string()),
            );
        }
    };

    let deserialized_account: Account = match serde_json::from_str(&serialized_account) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize account!".to_string()),
            );
        }
    };

    let deserialized_work_unit: WorkUnit = match serde_json::from_str(&serialized_work_unit) {
        Ok(ok) => ok,
        Err(_) => {
            return serialize_to_jstring(
                &env,
                Error::<()>::Unexpected("Couldn't deserialize wu!".to_string()),
            );
        }
    };

    string_to_jstring(
        &env,
        translate(execute_work(
            &deserialized_config,
            &deserialized_account,
            deserialized_work_unit,
        )),
    )
}
