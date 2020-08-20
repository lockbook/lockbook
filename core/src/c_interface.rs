use serde_json::json;
pub use sled::Db;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::str::FromStr;
use uuid::Uuid;

use crate::model::account::Account;
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::FileType;
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::repo::file_metadata_repo::{filter_from_str, Filter};
use crate::ExecuteWorkError;
use serde::Serialize;

fn json_c_string<T: Serialize>(value: T) -> *const c_char {
    CString::new(json!(value).to_string())
        .expect("Could not Rust String -> C String")
        .into_raw()
}

unsafe fn str_from_ptr(s: *const c_char) -> String {
    CStr::from_ptr(s)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn config_from_ptr(s: *const c_char) -> Config {
    Config {
        writeable_path: str_from_ptr(s),
    }
}

unsafe fn uuid_from_ptr(s: *const c_char) -> Uuid {
    Uuid::from_str(&str_from_ptr(s)).expect("Could not String -> Uuid")
}

unsafe fn file_type_from_ptr(s: *const c_char) -> FileType {
    FileType::from_str(&str_from_ptr(s)).expect("Could not String -> FileType")
}

unsafe fn filter_from_ptr(s: *const c_char) -> Option<Filter> {
    filter_from_str(&str_from_ptr(s)).expect("Could not String -> Option<Filter>")
}

unsafe fn work_unit_from_ptr(s: *const c_char) -> WorkUnit {
    serde_json::from_str(&str_from_ptr(s)).expect("Could not String -> WorkUnit")
}

unsafe fn account_from_ptr(s: *const c_char) -> Account {
    serde_json::from_str(&str_from_ptr(s)).expect("Could not String -> Account")
}

#[no_mangle]
pub unsafe extern "C" fn release_pointer(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

#[no_mangle]
pub unsafe extern "C" fn init_logger_safely() {
    crate::init_logger_safely();
}

#[no_mangle]
pub unsafe extern "C" fn create_account(
    writeable_path: *const c_char,
    username: *const c_char,
) -> *const c_char {
    json_c_string(crate::create_account(
        &config_from_ptr(writeable_path),
        &str_from_ptr(username),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn import_account(
    writeable_path: *const c_char,
    account_string: *const c_char,
) -> *const c_char {
    json_c_string(crate::import_account(
        &config_from_ptr(writeable_path),
        &str_from_ptr(account_string),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn export_account(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::export_account(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn get_account(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::get_account(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn create_file_at_path(
    writeable_path: *const c_char,
    path_and_name: *const c_char,
) -> *const c_char {
    json_c_string(crate::create_file_at_path(
        &config_from_ptr(writeable_path),
        &str_from_ptr(path_and_name),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn write_document(
    writeable_path: *const c_char,
    id: *const c_char,
    content: *const c_char,
) -> *const c_char {
    json_c_string(crate::write_document(
        &config_from_ptr(writeable_path),
        Uuid::from_str(&str_from_ptr(id)).expect("Could not String -> Uuid"),
        &DecryptedValue {
            secret: str_from_ptr(content),
        },
    ))
}

#[no_mangle]
pub unsafe extern "C" fn create_file(
    writeable_path: *const c_char,
    name: *const c_char,
    parent: *const c_char,
    file_type: *const c_char,
) -> *const c_char {
    json_c_string(crate::create_file(
        &config_from_ptr(writeable_path),
        &str_from_ptr(name),
        uuid_from_ptr(parent),
        file_type_from_ptr(file_type),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn get_root(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::get_root(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn get_file_by_path(
    writeable_path: *const c_char,
    path: *const c_char,
) -> *const c_char {
    json_c_string(crate::get_file_by_path(
        &config_from_ptr(writeable_path),
        &str_from_ptr(path),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn read_document(
    writeable_path: *const c_char,
    id: *const c_char,
) -> *const c_char {
    json_c_string(crate::read_document(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn list_paths(
    writeable_path: *const c_char,
    filter: *const c_char,
) -> *const c_char {
    json_c_string(crate::list_paths(
        &config_from_ptr(writeable_path),
        filter_from_ptr(filter),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn list_files(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::list_metas(&config_from_ptr(writeable_path)))
}

#[no_mangle]
pub unsafe extern "C" fn rename_file(
    writeable_path: *const c_char,
    id: *const c_char,
    new_name: *const c_char,
) -> *const c_char {
    json_c_string(crate::rename_file(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
        &str_from_ptr(new_name),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn list_metas(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::list_metas(&config_from_ptr(writeable_path)))
}

#[no_mangle]
pub unsafe extern "C" fn move_file(
    writeable_path: *const c_char,
    id: *const c_char,
    new_parent: *const c_char,
) -> *const c_char {
    json_c_string(crate::move_file(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
        uuid_from_ptr(new_parent),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn calculate_work(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::calculate_work(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn execute_work(
    writeable_path: *const c_char,
    work_unit: *const c_char,
) -> *const c_char {
    let config = &config_from_ptr(writeable_path);
    json_c_string(
        crate::get_account(config) // FIXME: @raayan Temporary to avoid passing key through FFI
            .map_err(ExecuteWorkError::BadAccount)
            .and_then(|acc| crate::execute_work(config, &acc, work_unit_from_ptr(work_unit))),
    )
}

#[no_mangle]
pub unsafe extern "C" fn sync_all(writeable_path: *const c_char) -> *const c_char {
    let config = &config_from_ptr(writeable_path);
    json_c_string(crate::sync_all(config))
}

#[no_mangle]
pub unsafe extern "C" fn set_last_synced(
    writeable_path: *const c_char,
    last_sync: u64,
) -> *const c_char {
    json_c_string(crate::set_last_synced(
        &config_from_ptr(writeable_path),
        last_sync,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn get_last_synced(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::get_last_synced(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn sync_all(writeable_path: *const c_char) -> *const c_char {
    json_c_string(crate::sync_all(&Config {
        writeable_path: str_from_ptr(writeable_path),
    }))
}
