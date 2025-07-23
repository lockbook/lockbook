use std::ffi::{CStr, CString, c_char};
use std::mem;
use std::path::PathBuf;

use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbErr;

use crate::lb_c_err::LbFfiErr;

pub(crate) fn cstring(from: String) -> *mut c_char {
    CString::new(from)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

pub(crate) fn cstring_array(from: Vec<String>) -> (*mut *mut c_char, usize) {
    carray(from.into_iter().map(cstring).collect::<Vec<*mut c_char>>())
}

pub(crate) fn carray<T>(mut from: Vec<T>) -> (*mut T, usize) {
    from.shrink_to_fit();
    let size = from.len();
    let ptr = from.as_mut_ptr();
    mem::forget(from);

    (ptr, size)
}

pub(crate) fn rvec<T>(ptr: *mut T, length: usize) -> Vec<T> {
    unsafe { Vec::from_raw_parts(ptr, length, length) }
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

pub(crate) fn r_opt_str<'a>(s: *const c_char) -> Option<&'a str> {
    unsafe { s.as_ref().map(|s| rstr(s)) }
}

pub(crate) fn rlb<'a>(clb: *mut Lb) -> &'a Lb {
    unsafe { clb.as_ref().unwrap() }
}

pub(crate) fn lb_err(err: LbErr) -> *mut LbFfiErr {
    Box::into_raw(Box::new(err.into()))
}

pub(crate) fn r_paths(paths: *const *const c_char, len: usize) -> Vec<PathBuf> {
    unsafe {
        (0..len)
            .map(|i| PathBuf::from(rstr(*paths.add(i))))
            .collect()
    }
}
