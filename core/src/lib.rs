#![feature(try_trait)]
extern crate reqwest;

#[macro_use]
extern crate log;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use serde_json::json;
pub use sled::Db;

use crate::client::ClientImpl;
use crate::model::state::Config;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::repo::file_repo::{FileRepo, FileRepoImpl};
use crate::repo::store::FsStore;
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};
use crate::service::sync_service::{FileSyncService, SyncService};
use std::marker::PhantomData;

pub mod client;
pub mod error_enum;
pub mod model;
pub mod repo;
pub mod service;

pub static API_LOC: &str = "http://lockbook.app:8000";
pub static BUCKET_LOC: &str = "https://locked.nyc3.digitaloceanspaces.com";
static DB_NAME: &str = "lockbook.sled";

pub type DefaultCrypto = RsaImpl;
pub type DefaultSymmetric = AesImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultClient = ClientImpl;
pub type DefaultClock = ClockImpl;
pub type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
pub type DefaultAccountService =
    AccountServiceImpl<DefaultCrypto, DefaultClient, DefaultAuthService>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultFileRepo = FileRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService =
    FileSyncService<DefaultFileMetadataRepo, DefaultFileRepo, DefaultClient, DefaultAuthService>;
pub type DefaultFileService =
    FileServiceImpl<DefaultFileMetadataRepo, DefaultFileRepo, DefaultFileEncryptionService>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_CREATE: &str = "FAILURE<META_CREATE>";
static FAILURE_FILE_GET: &str = "FAILURE<FILE_GET>";
// FIXME: Obviously a _temporary_ joke
const JUNK: &str =
    "/Users/raayanpillai/Library/Containers/com.raayanpillai.lockbook-client/Data/Documents/junk";

unsafe fn string_from_ptr(c_path: *const c_char) -> String {
    CStr::from_ptr(c_path)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn connect_db(
    c_path: *const c_char,
) -> (
    Option<Db>,
    Option<DefaultAccountService>,
    Option<DefaultSyncService>,
) {
    let path = string_from_ptr(c_path);
    let account_service = DefaultAccountService {
        encryption: PhantomData,
        client: PhantomData,
        auth: PhantomData,
        accountRepo: AccountRepo {
            store: Box::new(FsStore {
                config: Config {
                    writeable_path: path.clone(),
                },
            }),
        },
    };
    let sync_service = DefaultSyncService {
        metadatas: PhantomData,
        files: PhantomData,
        client: PhantomData,
        auth: PhantomData,
        accountsRepo: AccountRepo {
            store: Box::new(FsStore {
                config: Config {
                    writeable_path: path.clone(),
                },
            }),
        },
    };
    let config = Config {
        writeable_path: path.clone(),
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => (Some(db), Some(account_service), Some(sync_service)),
        Err(err) => {
            error!("DB connection failed! Error: {:?}", err);
            (None, None, None)
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn init_logger() {
    env_logger::init();
    info!("envvar RUST_LOG is {:?}", std::env::var("RUST_LOG"));
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> c_int {
    let path = string_from_ptr(c_path);
    debug!("Junk Path: {}", JUNK);

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
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return CString::new(FAILURE_DB).unwrap().into_raw(),
    };

    match acs.accountRepo.get_account() {
        Ok(account) => CString::new(account.username).unwrap().into_raw(),
        Err(err) => {
            error!("Account retrieval failed with error: {:?}", err);
            CString::new(FAILURE_ACCOUNT).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_account(c_path: *const c_char, c_username: *const c_char) -> c_int {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return 0,
    };

    let username = string_from_ptr(c_username);

    match acs.create_account(&db, &username) {
        Ok(_) => 1,
        Err(err) => {
            error!("Account creation failed with error: {:?}", err);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn sync_files(c_path: *const c_char) -> *mut c_char {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return CString::new(FAILURE_DB).unwrap().into_raw(),
    };

    match ss.sync(&db) {
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
    c_file_path: *const c_char,
) -> *mut c_char {
    let db = match connect_db(c_path) {
        (Some(db), _, _) => db,
        _ => return CString::new(FAILURE_DB).unwrap().into_raw(),
    };
    let file_name = string_from_ptr(c_file_name);
    let file_path = string_from_ptr(c_file_path);

    // match DefaultFileService::create(&db, &file_name, &file_path) {
    //     Ok(meta) => CString::new(json!(&meta).to_string()).unwrap().into_raw(),
    //     Err(err) => {
    //         error!("Failed to create file metadata! Error: {:?}", err);
    //         CString::new(FAILURE_META_CREATE).unwrap().into_raw()
    //     }
    // }
    CString::new("yeet").unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn get_file(c_path: *const c_char, c_file_id: *const c_char) -> *mut c_char {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return CString::new(FAILURE_DB).unwrap().into_raw(),
    };
    let file_id = string_from_ptr(c_file_id);

    // match DefaultFileService::get(&db, &file_id) {
    //     Ok(file) => CString::new(json!(&file).to_string()).unwrap().into_raw(),
    //     Err(err) => {
    //         error!("Failed to get file! Error: {:?}", err);
    //         CString::new(FAILURE_FILE_GET).unwrap().into_raw()
    //     }
    // }
    CString::new("yeet").unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn update_file(
    c_path: *const c_char,
    c_file_id: *const c_char,
    c_file_content: *const c_char,
) -> c_int {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return 0,
    };
    let file_id = string_from_ptr(c_file_id);
    let file_content = string_from_ptr(c_file_content);

    // match DefaultFileService::update(&db, &file_id, &file_content) {
    //     Ok(_) => 1,
    //     Err(err) => {
    //         error!("Failed to update file! Error: {:?}", err);
    //         0
    //     }
    // }
    1
}

#[no_mangle]
pub unsafe extern "C" fn purge_files(c_path: *const c_char) -> c_int {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return 0,
    };

    match DefaultFileMetadataRepo::get_all(&db) {
        Ok(metas) => metas.into_iter().for_each(|meta| {
            DefaultFileMetadataRepo::delete(&db, &meta.file_id).unwrap();
            DefaultFileRepo::delete(&db, &meta.file_id).unwrap();
            ()
        }),
        Err(err) => error!("Failed to delete file! Error: {:?}", err),
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn import_account(c_path: *const c_char, c_account: *const c_char) -> c_int {
    let (db, acs, ss) = match connect_db(c_path) {
        (Some(db), Some(acs), Some(ss)) => (db, acs, ss),
        _ => return 0,
    };

    let account_string = string_from_ptr(c_account);
    match acs.import_account(&db, &account_string) {
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
