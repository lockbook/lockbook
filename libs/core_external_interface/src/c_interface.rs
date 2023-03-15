use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde::Serialize;
use serde_json::json;

use lockbook_core::{Config, FileType, ShareMode, SupportedImageFormats, Uuid};

use crate::{get_all_error_variants, json_interface::translate, static_state};

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

unsafe fn config_from_ptr(path: *const c_char, logs: bool, colored_logs: bool) -> Config {
    Config { writeable_path: str_from_ptr(path), logs, colored_logs }
}

unsafe fn uuid_from_ptr(s: *const c_char) -> Uuid {
    str_from_ptr(s).parse().expect("Could not String -> Uuid")
}

unsafe fn file_type_from_ptr(s: *const c_char) -> FileType {
    str_from_ptr(s)
        .parse()
        .expect("Could not String -> FileType")
}

unsafe fn share_mode_from_ptr(s: *const c_char) -> ShareMode {
    str_from_ptr(s)
        .parse()
        .expect("Could not String -> ShareMode")
}

#[no_mangle]
pub extern "C" fn default_api_location() -> *const c_char {
    static C_DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net\0";

    C_DEFAULT_API_LOCATION.as_ptr() as *const c_char
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
pub unsafe extern "C" fn init(writeable_path: *const c_char, logs: bool) -> *const c_char {
    c_string(translate(static_state::init(&config_from_ptr(writeable_path, logs, true))))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_account(
    username: *const c_char, api_url: *const c_char, welcome_doc: bool,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.create_account(
            &str_from_ptr(username),
            &str_from_ptr(api_url),
            welcome_doc,
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn import_account(account_string: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.import_account(&str_from_ptr(account_string))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_account() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.export_account()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_account() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_account()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn delete_account() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.delete_account()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_file_at_path(path_and_name: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.create_at_path(&str_from_ptr(path_and_name))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn write_document(
    id: *const c_char, content: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.write_document(
            str_from_ptr(id).parse().expect("Could not String -> Uuid"),
            &str_from_ptr(content).into_bytes(),
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_file(
    name: *const c_char, parent: *const c_char, file_type: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.create_file(
            &str_from_ptr(name),
            uuid_from_ptr(parent),
            file_type_from_ptr(file_type),
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn create_link(
    name: *const c_char, parent: *const c_char, target: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(
            core.create_file(
                &str_from_ptr(name),
                uuid_from_ptr(parent),
                FileType::Link { target: uuid_from_ptr(target) },
            )
            .map(|_| ()),
        ),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_root() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_root()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_file_by_id(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_file_by_id(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_children(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_children(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_and_get_children_recursively(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_and_get_children_recursively(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_path_by_id(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_path_by_id(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_by_path(path: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_by_path(&str_from_ptr(path))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn delete_file(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.delete_file(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn read_document(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(
            core.read_document(uuid_from_ptr(id))
                .map(|d| String::from(String::from_utf8_lossy(&d))),
        ),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_drawing(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => {
            translate(core.export_drawing(uuid_from_ptr(id), SupportedImageFormats::Png, None))
        }
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn rename_file(id: *const c_char, new_name: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.rename_file(uuid_from_ptr(id), &str_from_ptr(new_name))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn list_metadatas() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.list_metadatas()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn move_file(id: *const c_char, new_parent: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.move_file(uuid_from_ptr(id), uuid_from_ptr(new_parent))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn calculate_work() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.calculate_work()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn sync_all() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.sync(None)),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_last_synced()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_last_synced_human_string() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_last_synced_human_string()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_usage() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_usage()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_uncompressed_usage() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_uncompressed_usage()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_drawing(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_drawing(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_local_changes() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_local_changes()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn upgrade_account_app_store(
    original_transaction_id: *const c_char, app_account_token: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.upgrade_account_app_store(
            str_from_ptr(original_transaction_id),
            str_from_ptr(app_account_token),
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn cancel_subscription() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.cancel_subscription()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn share_file(
    id: *const c_char, username: *const c_char, share_mode: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.share_file(
            uuid_from_ptr(id),
            &str_from_ptr(username),
            share_mode_from_ptr(share_mode),
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_pending_shares() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.get_pending_shares()),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn delete_pending_share(id: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.delete_pending_share(uuid_from_ptr(id))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn validate() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(
            // Map any warnings to Strings as well as any errors using Debug impl text.
            core.validate()
                .map(|warnings| {
                    warnings
                        .into_iter()
                        .map(|w| w.to_string())
                        .collect::<Vec<String>>()
                })
                .map_err(|err| err.to_string()),
        ),
        e => translate(e.map(|_| ())),
    })
}

// FOR INTEGRATION TESTS ONLY
/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_variants() -> *const c_char {
    json_c_string(get_all_error_variants())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ffi_api_location_matches() {
        unsafe {
            let ffi_val = std::ffi::CStr::from_ptr(super::default_api_location())
                .to_str()
                .expect("Could not C String -> Rust str");
            assert_eq!(crate::DEFAULT_API_LOCATION, ffi_val)
        }
    }
}
