use std::ffi::CString;

use crate::{lb_c_err::LbFfiErr, LbAccountRes, LbExportAccountQRRes, LbExportAccountRes, LbInitRes};

#[no_mangle]
pub extern "C" fn lb_free_err(err: *mut LbFfiErr) {
    unsafe {
        let err = *Box::from_raw(err);

        if !err.msg.is_null() {
            drop(CString::from_raw(err.msg));
        }

        if !err.trace.is_null() {
            drop(CString::from_raw(err.trace));
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_free_init(init: LbInitRes) {
    if !init.err.is_null() {
        lb_free_err(init.err);
    }

    if !init.lb.is_null() {
        unsafe { drop(Box::from_raw(init.lb)) };
    }
}

#[no_mangle]
pub extern "C" fn lb_free_account(acc: LbAccountRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

    if !acc.username.is_null() {
        unsafe { drop(CString::from_raw(acc.username)) }
    }

    if !acc.api_url.is_null() {
        unsafe { drop(CString::from_raw(acc.username)) }
    }
}

#[no_mangle]
pub extern "C" fn lb_free_export_account(acc: LbExportAccountRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

    if !acc.account_string.is_null() {
        unsafe { drop(CString::from_raw(acc.account_string)) }
    }
}

#[no_mangle]
pub extern "C" fn lb_free_export_account_qr(acc: LbExportAccountQRRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

}
