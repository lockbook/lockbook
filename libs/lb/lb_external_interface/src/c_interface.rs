use basic_human_duration::ChronoHumanDuration;
use crossbeam::channel::Sender;
use lazy_static::lazy_static;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::PathBuf;
use std::ptr::null;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use time::Duration;

use lb_rs::service::search_service::{SearchRequest, SearchResult, SearchType};
use lb_rs::{
    clock, Config, FileType, ImportStatus, ShareMode, SupportedImageFormats, SyncProgress,
    UnexpectedError, Uuid,
};

use crate::{get_all_error_variants, json_interface::translate, static_state, RankingWeights};

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
/// This function is used to log out and delete the user's data from the local filesystem.
/// Don't call it without warning the user to back up their private key.
#[no_mangle]
pub extern "C" fn logout_and_exit() -> ! {
    let core = static_state::get().expect("Could not get core");
    core.core.logout_and_exit();
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

pub type UpdateSyncStatus = extern "C" fn(*const c_char, *const c_char, f32);

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn sync_all(
    context: *const c_char, update_status: UpdateSyncStatus,
) -> *const c_char {
    let closure = move |sync_progress: SyncProgress| {
        update_status(
            context,
            c_string(sync_progress.msg),
            (sync_progress.progress as f32) / (sync_progress.total as f32),
        );
    };

    c_string(match static_state::get() {
        Ok(core) => translate(core.sync(Some(Box::new(closure)))),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn background_sync() -> *const c_char {
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
pub unsafe extern "C" fn export_file(
    id: *const c_char, destination: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.export_file(
            uuid_from_ptr(id),
            PathBuf::from(&str_from_ptr(destination)),
            true,
            None,
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn export_drawing_to_disk(
    id: *const c_char, destination: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.export_drawing_to_disk(
            uuid_from_ptr(id),
            SupportedImageFormats::Jpeg,
            None,
            &str_from_ptr(destination),
        )),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn import_files(
    sources: *const c_char, destination: *const c_char,
) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => {
            let sources = serde_json::from_str::<Vec<String>>(&str_from_ptr(sources))
                .expect("Could not convert Swift Array into Rust Array!")
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<PathBuf>>();

            translate(core.import_files(
                &sources,
                uuid_from_ptr(destination),
                &|_status: ImportStatus| println!("imported one file"),
            ))
        }
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn search_file_paths(input: *const c_char) -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.search_file_paths(&str_from_ptr(input))),
        e => translate(e.map(|_| ())),
    })
}

lazy_static! {
    static ref MAYBE_PATH_AND_CONTENT_SEARCH_TX: Arc<Mutex<Option<Sender<SearchRequest>>>> =
        Arc::new(Mutex::new(None));
    static ref MAYBE_PATH_SEARCH_TX: Arc<Mutex<Option<Sender<SearchRequest>>>> =
        Arc::new(Mutex::new(None));
}

fn send_search_request(request: SearchRequest, is_path_content_search: bool) -> *const c_char {
    let result = if is_path_content_search {
        MAYBE_PATH_AND_CONTENT_SEARCH_TX.lock()
    } else {
        MAYBE_PATH_SEARCH_TX.lock()
    }
    .map_err(|_| UnexpectedError::new("Could not get lock".to_string()))
    .and_then(|maybe_lock| {
        maybe_lock
            .clone()
            .ok_or_else(|| UnexpectedError::new("No search lock.".to_string()))
    })
    .and_then(|search_tx| search_tx.send(request).map_err(UnexpectedError::from));

    c_string(translate(result))
}

pub type UpdateSearchStatus = extern "C" fn(*const c_char, i32, *const c_char);

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn start_search(
    is_path_content_search: bool, context: *const c_char, update_status: UpdateSearchStatus,
) -> *const c_char {
    let (search_type, lock) = if is_path_content_search {
        (SearchType::PathAndContentSearch, MAYBE_PATH_AND_CONTENT_SEARCH_TX.lock())
    } else {
        (SearchType::PathSearch, MAYBE_PATH_SEARCH_TX.lock())
    };

    let results_rx = match lock {
        Ok(mut lock) => {
            let (results_rx, search_tx) = match static_state::get() {
                Ok(core) => {
                    let search_info = core.start_search(search_type);

                    (search_info.results_rx, search_info.search_tx)
                }
                Err(e) => return c_string(translate(Err::<(), _>(e))),
            };

            *lock = Some(search_tx);

            results_rx
        }
        Err(_) => return c_string(translate(Err::<(), _>("Cannot get search lock."))),
    };

    while let Ok(result) = results_rx.recv() {
        let (result_repr, content) = match result {
            SearchResult::Error(e) => return c_string(translate(Err::<(), _>(e))),
            SearchResult::StartOfSearch => (0, null()),
            SearchResult::FileNameMatch { .. } => {
                (1, c_string(serde_json::to_string(&result).unwrap()))
            }
            SearchResult::FileContentMatches { .. } => {
                (2, c_string(serde_json::to_string(&result).unwrap()))
            }
            SearchResult::EndOfSearch => (3, null()),
        };

        update_status(context, result_repr, content);
    }

    c_string(translate(Ok::<_, ()>(())))
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn search(
    query: *const c_char, is_path_content_search: bool,
) -> *const c_char {
    send_search_request(
        SearchRequest::Search { input: str_from_ptr(query) },
        is_path_content_search,
    )
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn end_search(is_path_content_search: bool) -> *const c_char {
    let result = send_search_request(SearchRequest::EndSearch, is_path_content_search);

    match if is_path_content_search {
        MAYBE_PATH_AND_CONTENT_SEARCH_TX.lock()
    } else {
        MAYBE_PATH_SEARCH_TX.lock()
    } {
        Ok(mut lock) => *lock = None,
        Err(_) => return c_string(translate(Err::<(), _>("Cannot get search lock."))),
    }

    result
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn suggested_docs() -> *const c_char {
    c_string(match static_state::get() {
        Ok(core) => translate(core.suggested_docs(RankingWeights::default())),
        e => translate(e.map(|_| ())),
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn time_ago(time_stamp: i64) -> *const c_char {
    c_string(if time_stamp != 0 {
        Duration::milliseconds(clock::get_time().0 - time_stamp)
            .format_human()
            .to_string()
    } else {
        "never".to_string()
    })
}

/// # Safety
///
/// Be sure to call `release_pointer` on the result of this function to free the data.
#[no_mangle]
pub unsafe extern "C" fn get_core_ptr() -> *mut c_void {
    let obj = static_state::get().expect("Could not get core").core;
    Box::into_raw(Box::new(obj)) as *mut c_void
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
            assert_eq!(lb_rs::DEFAULT_API_LOCATION, ffi_val)
        }
    }
}
