#![feature(try_trait)]
extern crate reqwest;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::path::Path;

use crate::account_api::{AccountApiImpl, AuthServiceImpl};
use crate::account_repo::AccountRepoImpl;
use crate::account_service::{AccountService, AccountServiceImpl};
use crate::crypto::RsaCryptoService;
use crate::db_provider::DiskBackedDB;
use crate::schema::SchemaCreatorImpl;
use crate::state::Config;

pub mod account;
pub mod account_api;
pub mod account_repo;
pub mod account_service;
pub mod crypto;
pub mod db_provider;
pub mod error_enum;
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
type DefaultAuthService = AuthServiceImpl;
type DefaultAcountService =
    AccountServiceImpl<DefaultDbProvider, DefaultCrypto, DefaultAcountRepo, DefaultAccountApi, DefaultAuthService>;

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
    let username = CStr::from_ptr(c_username)
        .to_str()
        .expect("Could not C String -> Rust String");

    let config = Config {
        writeable_path: "".to_string(),
    };

    match DefaultAcountService::create_account(config, username.to_string()) {
        Ok(_) => 0,
        Err(err) => {
            println!("Account creation failed with error: {:?}", err);
            1
        }
    }
}
