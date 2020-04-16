#![feature(try_trait)]
extern crate reqwest;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use serde_json::json;
use sled::Db;

use crate::auth_service::{AuthServiceImpl, AuthService};
use crate::client::ClientImpl;
use crate::crypto::RsaCryptoService;
use crate::model::state::Config;
use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
use crate::clock::{Clock, ClockImpl};

pub mod client;
pub mod crypto;
pub mod error_enum;
pub mod model;
pub mod repo;
pub mod service;
pub mod auth_service;
pub mod clock;

static API_LOC: &str = "http://lockbook.app:8000";
static DB_NAME: &str = "lockbook.sled";

type DefaultCrypto = RsaCryptoService;
type DefaultDbProvider = DiskBackedDB;
type DefaultClient = ClientImpl;
type DefaultAcountRepo = AccountRepoImpl;
type DefaultClock = ClockImpl;
type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
type DefaultAcountService = AccountServiceImpl<DefaultCrypto, DefaultAcountRepo, DefaultClient, DefaultAuthService>;
type DefaultFileMetadataRepo = FileMetadataRepoImpl;
type DefaultFileMetadataService =
FileMetadataServiceImpl<DefaultFileMetadataRepo, DefaultAcountRepo, DefaultClient>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_UPDATE: &str = "FAILURE<METADATA>";

fn info(msg: String) {
    println!("ℹ️ {}", msg)
}

fn debug(msg: String) {
    println!("🚧 {}", msg)
}

fn warn(msg: String) {
    println!("⚠️ {}", msg)
}

fn error(msg: String) {
    eprintln!("🛑 {}", msg)
}

fn fatal(msg: String) {
    eprintln!("🆘 {}", msg)
}

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
        max_auth_delay: 50,
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => Some(db),
        Err(err) => {
            error(format!("DB connection failed! Error: {:?}", err));
            None
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> c_int {
    let path = string_from_ptr(c_path);

    let db_path = path + "/" + DB_NAME;
    debug(format!("Checking if {:?} exists", db_path));
    if Path::new(db_path.as_str()).exists() {
        debug(format!("DB Exists!"));
        1
    } else {
        error(format!("DB Does not exist!"));
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

    match DefaultAcountRepo::get_account(&db) {
        Ok(account) => CString::new(account.username).unwrap().into_raw(),
        Err(err) => {
            error(format!("Account retrieval failed with error: {:?}", err));
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

    match DefaultAcountService::create_account(&db, username.to_string()) {
        Ok(_) => 1,
        Err(err) => {
            error(format!("Account creation failed with error: {:?}", err));
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn list_files(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    match DefaultFileMetadataService::update(&db) {
        Ok(files) => CString::new(json!(&files).to_string()).unwrap().into_raw(),
        Err(err) => {
            error(format!("Update metadata failed with error: {:?}", err));
            CString::new(json!([]).to_string()).unwrap().into_raw()
        }
    }
}
