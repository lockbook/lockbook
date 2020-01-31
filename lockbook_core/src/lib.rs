extern crate reqwest;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub unsafe extern "C" fn hello(to: *const c_char) -> *mut c_char {
    let c_str = CStr::from_ptr(to);

    CString::new(request())
        .unwrap()
        .into_raw()
}

fn request() -> String {
    reqwest::get("https://www.rust-lang.org")
        .expect("fail1")
        .text()
        .expect("fail2")
}

#[no_mangle]
pub unsafe extern "C" fn hello_release(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}
