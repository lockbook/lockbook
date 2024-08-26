use crate::*;
use lb_rs::AccountKey;

#[repr(C)]
pub struct LbAccount {
    username: *mut c_char,
    api_url: *mut c_char,
}

fn lb_account_new() -> LbAccount {
    LbAccount { username: null_mut(), api_url: null_mut() }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_account_free(a: LbAccount) {
    libc::free(a.username as *mut c_void);
    libc::free(a.api_url as *mut c_void);
}

#[repr(C)]
pub struct LbAccountResult {
    ok: LbAccount,
    err: LbError,
}

fn lb_account_result_new() -> LbAccountResult {
    LbAccountResult { ok: lb_account_new(), err: lb_error_none() }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_account_result_free(r: LbAccountResult) {
    if r.err.code == LbErrorCode::Success {
        lb_account_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_account_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_account_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_create_account(
    core: *mut c_void, username: *const c_char, api_url: *const c_char, welcome_doc: bool,
) -> LbAccountResult {
    let mut r = lb_account_result_new();
    match core!(core).create_account(rstr(username), rstr(api_url), welcome_doc) {
        Ok(acct) => {
            r.ok.username = cstr(acct.username);
            r.ok.api_url = cstr(acct.api_url);
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_account_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_account_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_import_account(
    core: *mut c_void, account_string: *const c_char,
) -> LbAccountResult {
    let mut r = lb_account_result_new();
    match core!(core).import_account(AccountKey::AccountString(rstr(account_string))) {
        Ok(acct) => {
            r.ok.username = cstr(acct.username);
            r.ok.api_url = cstr(acct.api_url);
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_string_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_export_account(core: *mut c_void) -> LbStringResult {
    let mut r = lb_string_result_new();
    match core!(core).export_account_string() {
        Ok(acct_str) => r.ok = cstr(acct_str),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_account_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_account_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_account(core: *mut c_void) -> LbAccountResult {
    let mut r = lb_account_result_new();
    match core!(core).get_account() {
        Ok(acct) => {
            r.ok.username = cstr(acct.username);
            r.ok.api_url = cstr(acct.api_url);
        }
        Err(err) => r.err = lberr(err),
    }
    r
}
