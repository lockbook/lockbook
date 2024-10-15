use std::ffi::CString;

use crate::{
    ffi_utils::rvec, lb_c_err::LbFfiErr, lb_file::LbFile, LbAccountRes, LbExportAccountQRRes,
    LbExportAccountRes, LbFileListRes, LbFileRes, LbInitRes,
};

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

    if !acc.qr.is_null() {
        drop(rvec(acc.qr, acc.qr_size));
    }
}

#[no_mangle]
pub extern "C" fn lb_free_file(file: LbFile) {
    unsafe {
        drop(CString::from_raw(file.name));
        drop(CString::from_raw(file.lastmod_by));
        let shares = rvec(file.shares.list, file.shares.count);
        for share in shares {
            drop(CString::from_raw(share.by));
            drop(CString::from_raw(share.with));
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_free_file_res(file_res: LbFileRes) {
    if !file_res.err.is_null() {
        lb_free_err(file_res.err);
    }

    if !file_res.file.id.is_nil() {
        lb_free_file(file_res.file);
    }
}

#[no_mangle]
pub extern "C" fn lb_free_file_list_res(files: LbFileListRes) {
    if !files.err.is_null() {
        lb_free_err(files.err);
    }

    if !files.list.list.is_null() {
        let files = rvec(files.list.list, files.list.count);
        for file in files {
            lb_free_file(file);
        }
    }
}
