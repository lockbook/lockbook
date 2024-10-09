use std::ffi::{c_char, CStr, CString};

use lb_rs::blocking::Lb;

pub(crate) fn cstring(from: String) -> *mut c_char {
    CString::new(from)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

pub(crate) fn bytes(mut from: Vec<u8>) -> *mut u8 {
    let ptr = from.as_mut_ptr();
    std::mem::forget(from);
    ptr
}

pub(crate) fn rstr<'a>(s: *const c_char) -> &'a str {
    unsafe { CStr::from_ptr(s).to_str().expect("*const char -> &str") }
}

pub(crate) fn rstring(s: *const c_char) -> String {
    unsafe {
        CStr::from_ptr(s)
            .to_str()
            .expect("*const char -> &str")
            .to_string()
    }
}

pub(crate) fn rlb<'a>(clb: *mut Lb) -> &'a Lb {
    unsafe { clb.as_ref().unwrap() }
}

pub(crate) fn mut_rlb<'a>(clb: *mut Lb) -> &'a mut Lb {
    unsafe { clb.as_mut().unwrap() }
}
