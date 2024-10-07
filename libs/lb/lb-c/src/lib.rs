use std::{
    ffi::{c_char, c_void},
    ptr,
};

use ffi_utils::{cstring, rlb, rstr, rstring};
use lb_c_err::LbFfiErr;
pub use lb_rs::{blocking::Lb, model::core_config::Config};

#[repr(C)]
pub struct LbInitRes {
    lb: *mut Lb,
    err: *mut LbFfiErr,
}

#[no_mangle]
pub extern "C" fn lb_init(writeable_path: *const c_char, logs: bool) -> LbInitRes {
    let writeable_path = rstring(writeable_path);

    let config = Config { logs, colored_logs: false, writeable_path };
    match Lb::init(config) {
        Ok(lb) => {
            let lb = Box::into_raw(Box::new(lb));
            LbInitRes { lb, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbInitRes { lb: ptr::null_mut(), err }
        }
    }
}

#[repr(C)]
pub struct LbAccountRes {
    username: *mut c_char,
    err: *mut LbFfiErr,
}

#[no_mangle]
pub extern "C" fn lb_create_account(
    lb: *mut Lb, username: *const c_char, api_url: *const c_char, welcome_doc: bool,
) -> LbAccountRes {
    let lb = rlb(lb);
    let username = rstr(username);
    let api_url = rstr(api_url);

    match lb.create_account(username, api_url, welcome_doc) {
        Ok(account) => {
            let username = cstring(account.username);
            LbAccountRes { username, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbAccountRes { username: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_import_account(
    lb: *mut Lb, key: *const c_char, api_url: *const c_char,
) -> LbAccountRes {
    let lb = rlb(lb);
    let key = rstr(key);
    let api_url = unsafe { api_url.as_ref().map(|url| rstr(url)) };

    match lb.import_account(key, api_url) {
        Ok(account) => {
            let username = cstring(account.username);
            LbAccountRes { username, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbAccountRes { username: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_logout_and_exit(lb: *mut Lb) {
    
}

mod ffi_utils;
mod lb_c_err;
mod mem_cleanup;
