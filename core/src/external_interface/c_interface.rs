use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::str::FromStr;

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use lockbook_models::file_metadata::FileType;

use crate::external_interface::json_interface::translate;
use crate::external_interface::static_state;
use crate::model::state::Config;
use crate::service::path_service::{filter_from_str, Filter};
use crate::{get_all_error_variants, SupportedImageFormats};

fn c_string(value: String) -> *const c_char {
    CString::new(value)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

fn json_c_string<T: Serialize>(value: T) -> *const c_char {
    c_string(json!(value).to_string())
}

unsafe fn str_from_ptr(s: *const c_char) -> String {
    CStr::from_ptr(s)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn config_from_ptr(s: *const c_char) -> Config {
    Config { writeable_path: str_from_ptr(s) }
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

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn release_pointer(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    let _ = CString::from_raw(s);
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn init_logger_safely(writeable_path: *const c_char) {
    if crate::init_logger(config_from_ptr(writeable_path).path()).is_ok() {
        debug!("Logger initialized!");
    }
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn init(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(static_state::init(&config_from_ptr(writeable_path))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_db_state(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_db_state(&config_from_ptr(writeable_path))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn migrate_db(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::migrate_db(&config_from_ptr(writeable_path))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_account(
    username: *const c_char, api_url: *const c_char,
) -> *const c_char {
    c_string(translate(
        static_state::get()
            .map(|core| core.create_account(&str_from_ptr(username), &str_from_ptr(api_url))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn import_account(account_string: *const c_char) -> *const c_char {
    c_string(translate(
        static_state::get().map(|core| core.import_account(&str_from_ptr(account_string))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_account() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.export_account())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_account() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_account())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_file_at_path(
    writeable_path: *const c_char, path_and_name: *const c_char,
) -> *const c_char {
    c_string(translate(crate::create_file_at_path(
        &config_from_ptr(writeable_path),
        &str_from_ptr(path_and_name),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn write_document(
    writeable_path: *const c_char, id: *const c_char, content: *const c_char,
) -> *const c_char {
    c_string(translate(crate::write_document(
        &config_from_ptr(writeable_path),
        Uuid::from_str(&str_from_ptr(id)).expect("Could not String -> Uuid"),
        &str_from_ptr(content).into_bytes(),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_file(
    writeable_path: *const c_char, name: *const c_char, parent: *const c_char,
    file_type: *const c_char,
) -> *const c_char {
    c_string(translate(crate::create_file(
        &config_from_ptr(writeable_path),
        &str_from_ptr(name),
        uuid_from_ptr(parent),
        file_type_from_ptr(file_type),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_root(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_root(&Config { writeable_path: str_from_ptr(writeable_path) })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_children(
    writeable_path: *const c_char, id: *const c_char,
) -> *const c_char {
    c_string(translate(crate::get_children(&config_from_ptr(writeable_path), uuid_from_ptr(id))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_file_by_path(
    writeable_path: *const c_char, path: *const c_char,
) -> *const c_char {
    c_string(translate(crate::get_file_by_path(
        &config_from_ptr(writeable_path),
        &str_from_ptr(path),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn delete_file(
    writeable_path: *const c_char, id: *const c_char,
) -> *const c_char {
    c_string(translate(crate::delete_file(&config_from_ptr(writeable_path), uuid_from_ptr(id))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn read_document(
    writeable_path: *const c_char, id: *const c_char,
) -> *const c_char {
    c_string(translate(
        crate::read_document(&config_from_ptr(writeable_path), uuid_from_ptr(id))
            .map(|d| String::from(String::from_utf8_lossy(&d))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_drawing(
    writeable_path: *const c_char, id: *const c_char,
) -> *const c_char {
    c_string(translate(crate::export_drawing(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
        SupportedImageFormats::Png,
        None,
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn list_paths(
    writeable_path: *const c_char, filter: *const c_char,
) -> *const c_char {
    c_string(translate(crate::list_paths(
        &config_from_ptr(writeable_path),
        filter_from_ptr(filter),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn rename_file(
    writeable_path: *const c_char, id: *const c_char, new_name: *const c_char,
) -> *const c_char {
    c_string(translate(crate::rename_file(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
        &str_from_ptr(new_name),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn list_metadatas(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::list_metadatas(&config_from_ptr(writeable_path))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn move_file(
    writeable_path: *const c_char, id: *const c_char, new_parent: *const c_char,
) -> *const c_char {
    c_string(translate(crate::move_file(
        &config_from_ptr(writeable_path),
        uuid_from_ptr(id),
        uuid_from_ptr(new_parent),
    )))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn calculate_work(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::calculate_work(&Config {
        writeable_path: str_from_ptr(writeable_path),
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn sync_all(writeable_path: *const c_char) -> *const c_char {
    let config = &config_from_ptr(writeable_path);
    c_string(translate(crate::sync_all(config, None)))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_last_synced(&Config {
        writeable_path: str_from_ptr(writeable_path),
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced_human_string(
    writeable_path: *const c_char,
) -> *const c_char {
    c_string(translate(crate::get_last_synced_human_string(&Config {
        writeable_path: str_from_ptr(writeable_path),
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_usage(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_usage(&Config { writeable_path: str_from_ptr(writeable_path) })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_uncomressed_usage(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_uncompressed_usage(&Config {
        writeable_path: str_from_ptr(writeable_path),
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_drawing(
    writeable_path: *const c_char, id: *const c_char,
) -> *const c_char {
    c_string(translate(crate::get_drawing(&config_from_ptr(writeable_path), uuid_from_ptr(id))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_local_changes(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(crate::get_local_changes(&Config {
        writeable_path: str_from_ptr(writeable_path),
    })))
}

// FOR INTEGRATION TESTS ONLY
/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_variants() -> *const c_char {
    json_c_string(get_all_error_variants())
}
