#![allow(non_snake_case)]

use std::path::Path;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint};
use sled::Db;
use uuid::Uuid;

use crate::{DB_NAME, DefaultAccountService, DefaultDbProvider, init_logger_safely};
use crate::client::Error;
use crate::model::api::NewAccountError;
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::{FileType, FileMetadata};
use crate::model::state::Config;
use crate::repo::account_repo::AccountRepoImpl;
use crate::repo::db_provider::DbProvider;
use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{AccountCreationError, AccountImportError};
use crate::service::account_service::AccountService;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};

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
        .expect("Couldn't read the path out of JNI!")
        .into();

    let username: String = env
        .get_string(jusername)
        .expect("Couldn't read the username out of JNI!")
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
                AccountCreationError::FolderError(_) => unexpected_error, // TODO added during files and folders (unhandled)
                AccountCreationError::MetadataRepoError(_) => unexpected_error, // TODO added during files and folders (unhandled)
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
        .expect("Couldn't read the path out of JNI!")
        .into();

    let account_string: String = env
        .get_string(jaccount_account)
        .expect("Couldn't read the account string out of JNI!")
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

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getRoot<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let root = FileMetadataRepoImpl::get_root(&db).expect("Couldn't access DB's root despite db being present!");

    let serialized_string = match serde_json::to_string(&root) {
        Ok(v) => v,
        _ => "".to_string() // change
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getChildren<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString,
    jparentuuid: JString,
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jparentuuid)
        .expect("Couldn't read the uuid string out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(e) => {
            return env.new_string(e.to_string()).expect("Couldn't create JString from rust string!")
        },
    };

    let children = FileMetadataRepoImpl::get_children(&db, uuid).expect("Could not read DB to get children!");

    let serialized_string = match serde_json::to_string(&children) {
        Ok(v) => v,
        _ => "".to_string()
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFileMetadata<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString,
    jfileuuid: JString,
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jfileuuid)
        .expect("Couldn't read the uuid string out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(e) => {
            return env.new_string(e.to_string()).expect("Couldn't create JString from rust string!")
        },
    };

    let file_metadata = FileMetadataRepoImpl::get(&db, uuid).expect("Couldn't read the DB to get a file!");

    let serialized_string = match serde_json::to_string(&file_metadata) {
        Ok(v) => v,
        _ => "".to_string()
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_getFile<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString,
    jfileuuid: JString,
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jfileuuid)
        .expect("Couldn't read the uuid string out of JNI!")
        .into();

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(e) => {
            return env.new_string(e.to_string()).expect("Couldn't create JString from rust string!")
        },
    };
    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let file = DocumentRepoImpl::get(&db, uuid).expect("Couldn't get the document from db and uuid!");

    let serialized_string = match serde_json::to_string(&file) {
        Ok(v) => v,
        _ => "".to_string()
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_insertFileFolder(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    jfilemetadata: JString,
) -> jint {
    let success = 0;
    let failure = 1;

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_file_metadata: String = env
        .get_string(jfilemetadata)
        .expect("Couldn't read file metadata out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let file_metadata: FileMetadata = serde_json::from_str(serialized_file_metadata.as_str()).expect("Couldn't serialize the file metadata!");

    match FileMetadataRepoImpl::insert(&db, &file_metadata) {
        Ok(()) => success,
        Err(_) => failure
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_deleteFileFolder(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    jfileuuid: JString,
) -> jint {
    let success = 0;
    let failure = 1;

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jfileuuid)
        .expect("Couldn't read the uuid string out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(_) => {
            return success
        },
    };

    match DocumentRepoImpl::delete(&db, uuid) {
        Ok(()) => success,
        Err(_) => failure
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_createFileFolder<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString,
    jparentuuid: JString,
    jfiletype: JString,
    jname: JString,
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jparentuuid)
        .expect("Couldn't read the parent folder out of JNI!")
        .into();

    let serialized_file_type: String = env
        .get_string(jfiletype)
        .expect("Couldn't read the file type out of JNI!")
        .into();

    let name: String = env
        .get_string(jname)
        .expect("Couldn't read the file name out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(e) => {
            return env.new_string(e.to_string()).expect("Couldn't create JString from rust string!")
        },
    };
    let file_type: FileType = serde_json::from_str(&serialized_file_type).expect("Couldn't deserialized the file type!");

    let file = FileServiceImpl
        ::<FileMetadataRepoImpl, DocumentRepoImpl, LocalChangesRepoImpl, AccountRepoImpl, FileEncryptionServiceImpl<RsaImpl, AesImpl>>::create(&db, name.as_str(), uuid, file_type).expect("Couldn't create a file!");

    let serialized_string = match serde_json::to_string(&file) {
        Ok(v) => v,
        _ => "".to_string()
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_writeToDocument(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    juuid: JString,
    jcontent: JString,
) -> jint {
    let success = 0;
    let failure = 1;

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(juuid)
        .expect("Couldn't read the file uuid out of JNI!")
        .into();

    let serialized_content: String = env
        .get_string(jcontent)
        .expect("Couldn't read the document content out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(_) => {
            return failure
        },
    };

    let content: DecryptedValue = serde_json::from_str(&serialized_content).expect("Couldn't deserialized the document content!");

    match FileServiceImpl
        ::<FileMetadataRepoImpl, DocumentRepoImpl, LocalChangesRepoImpl, AccountRepoImpl, FileEncryptionServiceImpl<RsaImpl, AesImpl>>::write_document(&db, uuid, &content) {
        Ok(()) => success,
        Err(_) => failure
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_readDocument<'a>(
    env: JNIEnv<'a>,
    _: JClass,
    jpath: JString,
    jfileuuid: JString,
) -> JString<'a> {
    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(jfileuuid)
        .expect("Couldn't read the uuid out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(e) => {
            return env.new_string(e.to_string()).expect("Couldn't create JString from rust string!")
        },
    };

    let document = FileServiceImpl
        ::<FileMetadataRepoImpl, DocumentRepoImpl, LocalChangesRepoImpl, AccountRepoImpl, FileEncryptionServiceImpl<RsaImpl, AesImpl>>::read_document(&db, uuid).expect("Couldn't read a document!");

    let serialized_string = match serde_json::to_string(&document) {
        Ok(v) => v,
        _ => "".to_string()
    };

    env.new_string(serialized_string).expect("Couldn't create JString from rust string!")
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_core_CoreKt_renameFileFolder(
    env: JNIEnv,
    _: JClass,
    jpath: JString,
    juuid: JString,
    jcontent: JString,
) -> jint {
    let success = 0;
    let failure = 1;

    let path: String = env
        .get_string(jpath)
        .expect("Couldn't read the path out of JNI!")
        .into();

    let serialized_uuid: String = env
        .get_string(juuid)
        .expect("Couldn't read the file uuid out of JNI!")
        .into();

    let serialized_content: String = env
        .get_string(jcontent)
        .expect("Couldn't read the document content out of JNI!")
        .into();

    let db = connect_db(&path).expect("Couldn't read the DB to get the root!");

    let uuid: Uuid = match Uuid::parse_str(&serialized_uuid) {
        Ok(v) => v,
        Err(_) => {
            return failure
        },
    };

    let content: DecryptedValue = serde_json::from_str(&serialized_content).expect("Couldn't deserialized the document content!");

    match FileServiceImpl
        ::<FileMetadataRepoImpl, DocumentRepoImpl, LocalChangesRepoImpl, AccountRepoImpl, FileEncryptionServiceImpl<RsaImpl, AesImpl>>::write_document(&db, uuid, &content) {
        Ok(()) => success,
        Err(_) => failure
    }
}