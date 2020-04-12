#![feature(try_trait)]
extern crate reqwest;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use crate::account_api::AccountApiImpl;
use crate::account_repo::AccountRepoImpl;
use crate::account_service::{AccountService, AccountServiceImpl};
use crate::crypto::RsaCryptoService;
use crate::db_provider::{DbProvider, DiskBackedDB};
use crate::file_metadata_repo::FileMetadataRepoImpl;
use crate::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
use crate::schema::SchemaCreatorImpl;
use crate::state::Config;

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

#[no_mangle]
pub unsafe extern "C" fn is_db_present(path_c: *const c_char) -> c_int {
    let path = CStr::from_ptr(path_c)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string();

    let db_path = path + "/" + DB_NAME;

    if Path::new(db_path.as_str()).exists() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_account(c_username: *const c_char) -> c_int {
    let config = Config {
        writeable_path: "".to_string(),
    };

    let db = match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => db,
        Err(_) => return 1,
    };

    let username = CStr::from_ptr(c_username)
        .to_str()
        .expect("Could not C String -> Rust String");

    match DefaultAcountService::create_account(&db, username.to_string()) {
        Ok(_) => 0,
        Err(err) => {
            println!("Account creation failed with error: {:?}", err);
            1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn list_files() -> *mut c_char {
    let config = Config {
        writeable_path: "".to_string(),
    };

    let db = match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => db,
        Err(_) => return CString::new("none").unwrap().into_raw(),
    };

    match DefaultFileMetadataService::get_all_files(&db) {
        Ok(files) => CString::new(serde_json::to_string(&files).unwrap())
            .unwrap()
            .into_raw(),
        Err(_) => CString::new("none").unwrap().into_raw(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn list_files_release(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}
