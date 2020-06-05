#![cfg(target_os = "android")]
#![allow(non_snake_case)]

use crate::DB_NAME;
use jni::objects::{JClass, JString};
use jni::sys::jboolean;
use jni::JNIEnv;
use std::path::Path;

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_isDbPresent(
    env: JNIEnv,
    _: JClass,
    input: JString,
) -> jboolean {
    let path: String = env
        .get_string(input)
        .expect("Couldn't read path out of JNI!")
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
