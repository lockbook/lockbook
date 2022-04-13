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
pub unsafe extern "C" fn init(writeable_path: *const c_char) -> *const c_char {
    c_string(translate(static_state::init(&config_from_ptr(writeable_path))))
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
pub unsafe extern "C" fn create_file_at_path(path_and_name: *const c_char) -> *const c_char {
    c_string(translate(
        static_state::get().map(|core| core.create_at_path(&str_from_ptr(path_and_name))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn write_document(
    id: *const c_char, content: *const c_char,
) -> *const c_char {
    c_string(translate(static_state::get().map(|core| {
        core.write_document(
            Uuid::from_str(&str_from_ptr(id)).expect("Could not String -> Uuid"),
            &str_from_ptr(content).into_bytes(),
        )
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_file(
    name: *const c_char, parent: *const c_char, file_type: *const c_char,
) -> *const c_char {
    c_string(translate(static_state::get().map(|core| {
        core.create_file(&str_from_ptr(name), uuid_from_ptr(parent), file_type_from_ptr(file_type))
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_root() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_root())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_children(id: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_children(uuid_from_ptr(id)))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_file_by_path(path: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_by_path(&str_from_ptr(path)))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn delete_file(id: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.delete_file(uuid_from_ptr(id)))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn read_document(id: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| {
        core.read_document(uuid_from_ptr(id))
            .map(|d| String::from(String::from_utf8_lossy(&d)))
    })))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_drawing(id: *const c_char) -> *const c_char {
    c_string(translate(
        static_state::get()
            .map(|core| core.export_drawing(uuid_from_ptr(id), SupportedImageFormats::Png, None)),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn list_paths(filter: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.list_paths(filter_from_ptr(filter)))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn rename_file(id: *const c_char, new_name: *const c_char) -> *const c_char {
    c_string(translate(
        static_state::get()
            .map(|core| core.rename_file(uuid_from_ptr(id), &str_from_ptr(new_name))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn list_metadatas() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.list_metadatas())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn move_file(id: *const c_char, new_parent: *const c_char) -> *const c_char {
    c_string(translate(
        static_state::get()
            .map(|core| core.move_file(uuid_from_ptr(id), uuid_from_ptr(new_parent))),
    ))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn calculate_work() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.calculate_work())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn sync_all() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.sync(None))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_last_synced())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced_human_string() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_last_synced_human_string())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_usage() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_usage())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_uncomressed_usage() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_uncompressed_usage())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_drawing(id: *const c_char) -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_drawing(uuid_from_ptr(id)))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_local_changes() -> *const c_char {
    c_string(translate(static_state::get().map(|core| core.get_local_changes())))
}

// FOR INTEGRATION TESTS ONLY
/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_variants() -> *const c_char {
    json_c_string(get_all_error_variants())
}
