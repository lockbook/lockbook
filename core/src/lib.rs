extern crate reqwest;

#[macro_use]
extern crate log;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use serde_json::json;
pub use sled::Db;

use crate::client::ClientImpl;
use crate::model::client_file_metadata::FileType::Document;
use crate::model::state::Config;
use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::repo::file_repo::{FileRepo, FileRepoImpl};
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};
use crate::service::sync_service::{FileSyncService, SyncService};

pub mod client;
pub mod error_enum;
pub mod model;
pub mod repo;
pub mod service;

mod android;

pub static API_LOC: &str = "http://lockbook.app:8000";
pub static BUCKET_LOC: &str = "https://locked.nyc3.digitaloceanspaces.com";
static DB_NAME: &str = "lockbook.sled";

pub type DefaultCrypto = RsaImpl;
pub type DefaultSymmetric = AesImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultClient = ClientImpl;
pub type DefaultAccountRepo = AccountRepoImpl;
pub type DefaultClock = ClockImpl;
pub type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultFileRepo = FileRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultFileRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultFileRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_CREATE: &str = "FAILURE<META_CREATE>";
static FAILURE_FILE_GET: &str = "FAILURE<FILE_GET>";

unsafe fn string_from_ptr(c_path: *const c_char) -> String {
    CStr::from_ptr(c_path)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn connect_db(c_path: *const c_char) -> Option<Db> {
    let path = string_from_ptr(c_path);
    let config = Config {
        writeable_path: path,
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => Some(db),
        Err(err) => {
            error!("DB connection failed! Error: {:?}", err);
            None
        }
    }
}

pub fn init_logger_safely() {
    env_logger::init();
    info!("envvar RUST_LOG is {:?}", std::env::var("RUST_LOG"));
}

#[no_mangle]
pub unsafe extern "C" fn init_logger() {
    init_logger_safely()
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> c_int {
    let path = string_from_ptr(c_path);

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
pub unsafe extern "C" fn release_pointer(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

#[no_mangle]
pub unsafe extern "C" fn get_account(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => CString::new(account.username).unwrap().into_raw(),
        Err(err) => {
            error!("Account retrieval failed with error: {:?}", err);
            CString::new(FAILURE_ACCOUNT).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_account(c_path: *const c_char, c_username: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };

    let username = string_from_ptr(c_username);

    match DefaultAccountService::create_account(&db, &username) {
        Ok(_) => 1,
        Err(err) => {
            error!("Account creation failed with error: {:?}", err);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn sync_files(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    match DefaultSyncService::sync(&db) {
        Ok(metas) => CString::new(json!(&metas).to_string()).unwrap().into_raw(),
        Err(err) => {
            error!("Update metadata failed with error: {:?}", err);
            CString::new(json!([]).to_string()).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_file(
    c_path: *const c_char,
    c_file_name: *const c_char,
    c_file_parent_id: *const c_char, // TODO @raayan add type?
) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };
    let file_name = string_from_ptr(c_file_name);
    let file_parent_id = string_from_ptr(c_file_parent_id);

    match DefaultFileService::create(
        &db,
        &file_name,
        serde_json::from_str(&file_parent_id).unwrap(),
        Document, // TODO @raayan
    ) {
        Ok(meta) => CString::new(json!(&meta).to_string()).unwrap().into_raw(),
        Err(err) => {
            error!("Failed to create file metadata! Error: {:?}", err);
            CString::new(FAILURE_META_CREATE).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_file(c_path: *const c_char, c_file_id: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };
    let file_id = string_from_ptr(c_file_id);

    match DefaultFileService::get(&db, serde_json::from_str(&file_id).unwrap()) {
        Ok(file) => CString::new(json!(&file).to_string()).unwrap().into_raw(),
        Err(err) => {
            error!("Failed to get file! Error: {:?}", err);
            CString::new(FAILURE_FILE_GET).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn update_file(
    c_path: *const c_char,
    c_file_id: *const c_char,
    c_file_content: *const c_char,
) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    let file_id = string_from_ptr(c_file_id);
    let file_content = string_from_ptr(c_file_content);

    match DefaultFileService::update(&db, serde_json::from_str(&file_id).unwrap(), &file_content) {
        Ok(_) => 1,
        Err(err) => {
            error!("Failed to update file! Error: {:?}", err);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn purge_files(c_path: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    match DefaultFileMetadataRepo::get_all(&db) {
        Ok(metas) => metas.into_iter().for_each(|meta| {
            DefaultFileMetadataRepo::delete(&db, meta.id).unwrap();
            DefaultFileRepo::delete(&db, meta.id).unwrap();
        }),
        Err(err) => error!("Failed to delete file! Error: {:?}", err),
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn import_account(c_path: *const c_char, c_account: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    let account_string = string_from_ptr(c_account);
    match DefaultAccountService::import_account(&db, &account_string) {
        Ok(acc) => {
            debug!("Loaded account: {:?}", acc);
            1
        }
        Err(err) => {
            error!("Failed to delete file! Error: {:?}", err);
            0
        }
    }
}
