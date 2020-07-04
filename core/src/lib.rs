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
use crate::model::crypto::DecryptedValue;
use crate::model::state::Config;
use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};
use crate::service::sync_service::{FileSyncService, SyncService};
use uuid::Uuid;

pub mod client;
pub mod model;
pub mod repo;
pub mod service;

mod android;

pub static API_LOC: &str = "http://lockbook_server:8000";
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
pub type DefaultDocumentRepo = DocumentRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_CREATE: &str = "FAILURE<META_CREATE>";
static FAILURE_FILE_GET: &str = "FAILURE<FILE_GET>";
static FAILURE_ROOT_GET: &str = "FAILURE<ROOT_GET>";
static FAILURE_UUID_UNWRAP: &str = "FAILURE<UUID_UNWRAP>";

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
            error!("DB connection failed! Error: {:?}", err); // TEMP HERE
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
pub unsafe extern "C" fn get_root(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    let out = match DefaultFileMetadataRepo::get_root(&db) {
        Ok(Some(root)) => root.id.to_string(),
        Ok(None) => FAILURE_ROOT_GET.to_string(),
        Err(err) => {
            error!("Failed to get root! Error: {:?}", err);
            FAILURE_ROOT_GET.to_string()
        }
    };

    CString::new(out.as_str()).unwrap().into_raw()
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

    let file_parent_uuid: Uuid = match Uuid::parse_str(&file_parent_id) {
        Ok(uuid) => uuid,
        Err(err) => {
            error!("Failed to create file metadata! Error: {:?}", err);
            return CString::new(FAILURE_UUID_UNWRAP).unwrap().into_raw();
        }
    };

    match DefaultFileService::create(
        &db,
        &file_name,
        file_parent_uuid,
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

    match DefaultFileService::read_document(&db, serde_json::from_str(&file_id).unwrap()) {
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
    let file_content = DecryptedValue {
        secret: string_from_ptr(c_file_content),
    };

    match DefaultFileService::write_document(
        &db,
        serde_json::from_str(&file_id).unwrap(),
        &file_content,
    ) {
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
            DefaultFileMetadataRepo::actually_delete(&db, meta.id).unwrap();
            DefaultDocumentRepo::delete(&db, meta.id).unwrap();
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
