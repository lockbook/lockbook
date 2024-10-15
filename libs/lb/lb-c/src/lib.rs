use std::{
    ffi::{c_char, c_uchar},
    fs, process, ptr,
};

use ffi_utils::{carray, cstring, rlb, rstr, rstring};
use lb_c_err::LbFfiErr;
use lb_file::{LbFile, LbFileType};
use lb_rs::Uuid;
pub use lb_rs::{blocking::Lb, model::core_config::Config};

#[repr(C)]
pub struct LbInitRes {
    err: *mut LbFfiErr,
    lb: *mut Lb,
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
    err: *mut LbFfiErr,
    username: *mut c_char,
    api_url: *mut c_char,
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
            let api_url = cstring(account.api_url);
            LbAccountRes { username, api_url, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbAccountRes { username: ptr::null_mut(), api_url: ptr::null_mut(), err }
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
            let api_url = cstring(account.api_url);
            LbAccountRes { username, api_url, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbAccountRes { username: ptr::null_mut(), api_url: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_get_account(lb: *mut Lb) -> LbAccountRes {
    let lb = rlb(lb);

    match lb.get_account() {
        Ok(account) => {
            let username = cstring(account.username.clone());
            let api_url = cstring(account.api_url.clone());
            LbAccountRes { username, api_url, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbAccountRes { username: ptr::null_mut(), api_url: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_delete_account(lb: *mut Lb) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.delete_account() {
        Ok(_) => ptr::null_mut(),
        Err(err) => Box::into_raw(Box::new(err.into())),
    }
}

#[no_mangle]
pub extern "C" fn lb_logout_and_exit(lb: *mut Lb) {
    let lb = rlb(lb);
    fs::remove_dir_all(&lb.get_config().writeable_path).unwrap();
    process::exit(0);
}

#[repr(C)]
pub struct LbExportAccountRes {
    err: *mut LbFfiErr,
    account_string: *mut c_char,
}

#[no_mangle]
pub extern "C" fn lb_export_account_private_key(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_private_key() {
        Ok(account_key) => {
            let account_string = cstring(account_key);
            LbExportAccountRes { account_string, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountRes { account_string: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_export_account_phrase(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_phrase() {
        Ok(account_phrase) => {
            let account_string = cstring(account_phrase);
            LbExportAccountRes { account_string, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountRes { account_string: ptr::null_mut(), err }
        }
    }
}

#[repr(C)]
pub struct LbExportAccountQRRes {
    err: *mut LbFfiErr,
    qr: *mut c_uchar,
    qr_size: usize,
}

#[no_mangle]
pub extern "C" fn lb_export_account_qr(lb: *mut Lb) -> LbExportAccountQRRes {
    let lb = rlb(lb);

    match lb.export_account_qr() {
        Ok(account_qr) => {
            let (qr, qr_size) = carray(account_qr);
            LbExportAccountQRRes { qr, qr_size, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountQRRes { qr: ptr::null_mut(), qr_size: 0, err }
        }
    }
}

#[repr(C)]
pub struct LbFileRes {
    err: *mut LbFfiErr,
    file: LbFile,
}

#[no_mangle]
pub extern "C" fn lb_create_file(
    lb: *mut Lb, name: *const c_char, parent: Uuid, file_type: LbFileType,
) -> LbFileRes {
    let lb = rlb(lb);
    let name = rstr(name);
    let file_type = file_type.into();

    match lb.create_file(name, &parent, file_type) {
        Ok(f) => LbFileRes { err: ptr::null_mut(), file: f.into() },
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbFileRes { err, file: LbFile::default() }
        }
    }
}

mod ffi_utils;
mod lb_c_err;
mod lb_file;
mod mem_cleanup;
