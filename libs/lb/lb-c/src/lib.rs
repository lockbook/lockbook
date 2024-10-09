use std::{
    ffi::{c_char, c_void},
    ptr,
};

use ffi_utils::{bytes, cstring, mut_rlb, rlb, rstr, rstring};
use lb_c_err::{LbEC, LbFfiErr};
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
    api_url: *mut c_char,
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
    let lb = mut_rlb(lb);

    match lb.delete_account() {
        Ok(_) => ptr::null_mut(),
        Err(err) => Box::into_raw(Box::new(err.into()))
    }
}

#[no_mangle]
pub extern "C" fn lb_logout_and_exit(lb: *mut Lb) {
    let lb = rlb(lb);
    std::fs::remove_dir_all(&lb.get_config().writeable_path).unwrap();
    std::process::exit(0);
}

#[repr(C)]
pub struct LbExportAccountRes {
    account_key: *mut c_char,
    account_phrase: *mut c_char,
    account_qr: *mut u8,
    err: *mut LbFfiErr,
}

#[no_mangle]
pub extern "C" fn lb_export_account_private_key(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_private_key() {
        Ok(account_key) => {
            let account_key = cstring(account_key);
            LbExportAccountRes { account_key, account_phrase: ptr::null_mut(), account_qr: ptr::null_mut(), err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountRes { account_key: ptr::null_mut(), account_phrase: ptr::null_mut(), account_qr: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_export_account_phrase(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_phrase() {
        Ok(account_phrase) => {
            let account_phrase = cstring(account_phrase);
            LbExportAccountRes { account_key: ptr::null_mut(), account_phrase, account_qr: ptr::null_mut(), err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountRes { account_key: ptr::null_mut(), account_phrase: ptr::null_mut(), account_qr: ptr::null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_export_account_qr(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_qr() {
        Ok(account_qr) => {
            let account_qr = bytes(account_qr);
            LbExportAccountRes { account_key: ptr::null_mut(), account_phrase: ptr::null_mut(), account_qr, err: ptr::null_mut() }
        }
        Err(err) => {
            let err = Box::into_raw(Box::new(err.into()));
            LbExportAccountRes { account_key: ptr::null_mut(), account_phrase: ptr::null_mut(), account_qr: ptr::null_mut(), err }
        }
    }
}

#[repr(C)]
pub struct LbUsageMetricsRes {
    usages: *mut LbFileUsage,
    usages_size: u64,

    server_usage: *mut LbUsageItemMetric,
    data_cap: *mut LbUsageItemMetric,
    err: *mut LbFfiErr,
}

#[repr(C)]
pub struct LbFileUsage {
    file_id: *mut c_char,
    size_bytes: u64
}

#[repr(C)]
pub struct LbUsageItemMetric {
    exact: u64,
    readable: *mut c_char
}

// #[no_mangle]
// pub extern "C" fn lb_get_usage(lb: *mut Lb) -> LbUsageMetricsRes {
//     let lb = rlb(lb);

//     match lb.get_usage() {
//         Ok(account_qr) => {
//             let account_qr = bytes(account_qr);
//             LbExportAccountRes { account_key: ptr::null_mut(), account_phrase: ptr::null_mut(), account_qr, err: ptr::null_mut() }
//         }
//         Err(err) => {
//             let err = Box::into_raw(Box::new(err.into()));
//             LbUsageMetricsRes {
//                 usages: ptr::null_mut(),
//                 usages_size: 0,
//                 server_usage: 
//             }
//         }
//     }
// }

mod ffi_utils;
mod lb_c_err;
mod mem_cleanup;
