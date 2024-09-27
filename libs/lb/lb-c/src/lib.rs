use std::{ffi::c_char, ptr};

use ffi_utils::rstring;
use lb_c_err::LbFfiErr;
use lb_rs::{blocking::Lb, model::core_config::Config};

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
        },
    }
}

mod ffi_utils;
mod lb_c_err;

