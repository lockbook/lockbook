#![allow(non_snake_case)]

use crate::client::Error;
use crate::model::api::NewAccountError;
use crate::model::state::Config;
use crate::repo::db_provider::DbProvider;
use crate::service::account_service::AccountService;
use crate::service::account_service::{AccountCreationError, AccountImportError};
use crate::{init_logger_safely, DefaultAccountService, DefaultDbProvider, DB_NAME};
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint};
use jni::JNIEnv;
use sled::Db;
use std::path::Path;

fn connect_db(path: &str) -> Option<Db> {
    let config = Config {
        writeable_path: String::from(path),
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => Some(db),
        Err(err) => {
            error!("DB connection failed! Error: {:?}", err);
            None
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_initLogger(_env: JNIEnv, _: JClass) {
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

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createAccount(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    jusername: JString,
) -> jint {
    // Error codes for this function
    let success = 0; // should handle
    let no_db = 1;
    let crypto_error = 2;
    let io_error = 3;
    let network_error = 4; // should handle
    let unexpected_error = 5;
    let username_taken = 6; // should handle

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read path out of JNI!")
        .into();

    let username: String = env
        .get_string(jusername)
        .expect("Couldn't read path out of JNI!")
        .into();

    let db = match connect_db(&path) {
        None => return no_db,
        Some(db) => db,
    };

    match DefaultAccountService::create_account(&db, &username) {
        Ok(_) => success,
        Err(err) => {
            error! {"Error while generating account! {:?}", &err}
            match err {
                AccountCreationError::KeyGenerationError(_) => crypto_error,
                AccountCreationError::PersistenceError(_) => io_error,
                AccountCreationError::ApiError(api_err) => match api_err {
                    Error::<NewAccountError>::SendFailed(_) => network_error,
                    Error::<NewAccountError>::Api(real_api_error) => match real_api_error {
                        NewAccountError::UsernameTaken => username_taken,
                        _ => unexpected_error,
                    },
                    _ => unexpected_error,
                },
                AccountCreationError::KeySerializationError(_) => unexpected_error,
                AccountCreationError::AuthGenFailure(_) => unexpected_error,
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_importAccount(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    jaccount_account: JString,
) -> jint {
    // Error codes for this function
    let success = 0; // should handle
    let no_db = 1;
    let account_string_invalid = 2; // should handle
    let io_err = 3;

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read path out of JNI!")
        .into();

    let account_string: String = env
        .get_string(jaccount_account)
        .expect("Couldn't read path out of JNI!")
        .into();

    let db = match connect_db(&path) {
        None => return no_db,
        Some(db) => db,
    };

    match DefaultAccountService::import_account(&db, &account_string) {
        Ok(_) => success,
        Err(err) => match err {
            AccountImportError::AccountStringCorrupted(_) => account_string_invalid,
            AccountImportError::AccountStringFailedToDeserialize(_) => account_string_invalid,
            AccountImportError::PersistenceError(_) => io_err,
            AccountImportError::InvalidPrivateKey(_) => account_string_invalid,
        },
    }
}
