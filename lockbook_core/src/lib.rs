extern crate reqwest;

mod state;
mod db_provider;
mod crypto;
mod account_repo;
mod account_service;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use crate::account_service::AccountServiceImpl;
use crate::crypto::RsaCryptoService;
use crate::db_provider::DbProviderImpl;
use crate::account_repo::AccountRepoImpl;

static DB_NAME: &str = "lockbook.db3";

type DefaultCrypto = RsaCryptoService;
type DefaultDbProvider = DbProviderImpl;

#[no_mangle]
pub unsafe extern "C" fn is_db_present(path_c: *const c_char) -> c_int {
    let path =
        CStr::from_ptr(path_c).to_str()
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
    let username = CStr::from_ptr(c_username).to_str()
        .expect("Could not C String -> Rust String");

    println!("username: {}", username);

    1
}
