#![feature(try_trait)]
extern crate reqwest;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use crate::account_api::AccountApiImpl;
use crate::account_repo::{AccountRepo, AccountRepoImpl};
use crate::account_service::{AccountService, AccountServiceImpl};
use crate::crypto::RsaCryptoService;
use crate::db_provider::{DbProvider, DiskBackedDB};
use crate::file_metadata_repo::FileMetadataRepoImpl;
use crate::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
use crate::schema::SchemaCreatorImpl;
use crate::state::Config;
use rusqlite::Connection;
use serde_json::json;

pub mod account;
pub mod account_api;
pub mod account_repo;
pub mod account_service;
pub mod crypto;
pub mod db_provider;
pub mod error_enum;
pub mod file_metadata;
pub mod file_metadata_repo;
pub mod file_metadata_service;
pub mod lockbook_api;
pub mod schema;
pub mod state;

static API_LOC: &str = "http://lockbook.app:8000";
static DB_NAME: &str = "lockbook.db3";

type DefaultCrypto = RsaCryptoService;
type DefaultSchema = SchemaCreatorImpl;
type DefaultDbProvider = DiskBackedDB<DefaultSchema>;
type DefaultAcountRepo = AccountRepoImpl;
type DefaultAccountApi = AccountApiImpl;
type DefaultAcountService = AccountServiceImpl<DefaultCrypto, DefaultAcountRepo, DefaultAccountApi>;
type DefaultFileMetadataRepo = FileMetadataRepoImpl;
type DefaultFileMetadataService =
    FileMetadataServiceImpl<DefaultFileMetadataRepo, DefaultAcountRepo>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_UPDATE: &str = "FAILURE<METADATA>";

unsafe fn string_from_ptr(c_path: *const c_char) -> String {
    CStr::from_ptr(c_path)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn connect_db(c_path: *const c_char) -> Option<Connection> {
    let path = string_from_ptr(c_path);
    let config = Config {
        writeable_path: path,
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => Some(db),
        Err(err) => {
            println!("❤️ DB connection failed! Error: {:?}", err);
            None
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> c_int {
    let path = string_from_ptr(c_path);

    let db_path = path + "/" + DB_NAME;
    println!("💚 Checking if {:?} exists", db_path);
    if Path::new(db_path.as_str()).exists() {
        println!("💚 DB Exists!");
        1
    } else {
        println!("❤️ DB Does not exist!");
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
            println!("❤️ Account retrieval failed with error: {:?}", err);
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
            println!("❤️ Account creation failed with error: {:?}", err);
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
            println!("❤️ Update metadata failed with error: {:?}", err);
            CString::new(json!([]).to_string()).unwrap().into_raw()
        }
    }
}
