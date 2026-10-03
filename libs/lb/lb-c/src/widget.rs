use std::ffi::c_char;
use std::path::Path;
use std::ptr::null_mut;

use crate::LbFileListRes;
use crate::ffi_utils::{lb_err, rstr};
use lb_rs::service::pin::read_pinned_documents;

/// Reads a fresh local snapshot for a widget without starting sync or event subscriptions.
/// Free the returned list with `lb_free_file_list_res`.
#[unsafe(no_mangle)]
pub extern "C" fn lb_widget_pinned_documents(writeable_path: *const c_char) -> LbFileListRes {
    match read_pinned_documents(Path::new(rstr(writeable_path))) {
        Ok(files) => LbFileListRes { err: null_mut(), list: files.into() },
        Err(error) => LbFileListRes { err: lb_err(error), list: Default::default() },
    }
}
