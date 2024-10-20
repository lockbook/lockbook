use std::{
    ffi::{c_char, CStr, CString},
    mem,
};

use lb_rs::{blocking::Lb, model::errors::LbErr};

use crate::lb_c_err::LbFfiErr;

pub(crate) fn cstring(from: String) -> *mut c_char {
    CString::new(from)
        .expect("Could not Rust String -> C String")
        .into_raw()
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

pub(crate) fn rlb<'a>(clb: *mut Lb) -> &'a Lb {
    unsafe { clb.as_ref().unwrap() }
}

pub(crate) fn lb_err(err: LbErr) -> *mut LbFfiErr {
    Box::into_raw(Box::new(err.into()))
}