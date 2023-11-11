use lb_rs::WorkUnit;

use crate::*;

#[repr(C)]
pub struct LbWorkCalc {
    units: *mut LbWorkUnit,
    num_units: usize,
    last_server_update_at: u64,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_work_calc_index(wc: LbWorkCalc, i: usize) -> *mut LbWorkUnit {
    wc.units.add(i)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_work_calc_free(wc: LbWorkCalc) {
    Vec::from_raw_parts(wc.units, wc.num_units, wc.num_units);
}

#[repr(C)]
pub struct LbCalcWorkResult {
    ok: LbWorkCalc,
    err: LbError,
}

#[repr(C)]
pub struct LbWorkUnit {
    pub typ: LbWorkUnitType,
    pub id: [u8; UUID_LEN],
}

#[repr(C)]
pub enum LbWorkUnitType {
    Local,
    Server,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_calc_work_result_free(r: LbCalcWorkResult) {
    if r.err.code == LbErrorCode::Success {
        lb_work_calc_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_calc_work_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_work_calc_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_calculate_work(core: *mut c_void) -> LbCalcWorkResult {
    let mut r = LbCalcWorkResult {
        ok: LbWorkCalc { units: null_mut(), num_units: 0, last_server_update_at: 0 },
        err: lb_error_none(),
    };
    match core!(core).calculate_work() {
        Ok(work) => {
            let mut list = Vec::with_capacity(work.work_units.len());
            for wu in work.work_units {
                let typ = match wu {
                    WorkUnit::LocalChange { .. } => LbWorkUnitType::Local,
                    WorkUnit::ServerChange { .. } => LbWorkUnitType::Server,
                };
                let id = match wu {
                    WorkUnit::LocalChange(id) => id,
                    WorkUnit::ServerChange(id) => id,
                }
                .into_bytes();
                list.push(LbWorkUnit { typ, id });
            }
            let mut list = std::mem::ManuallyDrop::new(list);
            r.ok.units = list.as_mut_ptr();
            r.ok.num_units = list.len();
            r.ok.last_server_update_at = work.latest_server_ts;
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

#[repr(C)]
pub struct LbSyncProgress {
    total: u64,
    progress: u64,
    msg: *mut c_char,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_sync_progress_free(sp: LbSyncProgress) {
    libc::free(sp.msg as *mut c_void)
}

pub type LbSyncProgressCallback = unsafe extern "C" fn(LbSyncProgress, *mut c_void);

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_sync_all(
    core: *mut c_void, progress: LbSyncProgressCallback, user_data: *mut c_void,
) -> LbError {
    match core!(core).sync(Some(Box::new(move |sp| {
        let c_sp = LbSyncProgress {
            total: sp.total as u64,
            progress: sp.progress as u64,
            msg: cstr(sp.msg),
        };
        progress(c_sp, user_data);
    }))) {
        Ok(_work) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

#[repr(C)]
pub struct LbInt64Result {
    ok: i64,
    err: LbError,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_int64_result_free(r: LbInt64Result) {
    if r.err.code != LbErrorCode::Success {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_int64_result_free` to avoid a memory leak.
/// Alternatively, the `err` value can be passed to `lb_error_free` if there's an error.
#[no_mangle]
pub unsafe extern "C" fn lb_get_last_synced(core: *mut c_void) -> LbInt64Result {
    let mut r = LbInt64Result { ok: 0, err: lb_error_none() };
    match core!(core).get_last_synced() {
        Ok(v) => r.ok = v,
        Err(err) => {
            r.err.msg = cstr(format!("{:?}", err));
            r.err.code = LbErrorCode::Unexpected;
        }
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_string_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_last_synced_human_string(core: *mut c_void) -> LbStringResult {
    let mut r = lb_string_result_new();
    match core!(core).get_last_synced_human_string() {
        Ok(acct_str) => r.ok = cstr(acct_str),
        Err(err) => r.err = lberr_unexpected(err),
    }
    r
}

#[repr(C)]
pub struct LbUsage {
    usages: *mut LbFileUsage,
    num_usages: usize,
    server_usage: LbUsageItemMetric,
    data_cap: LbUsageItemMetric,
}

#[repr(C)]
pub struct LbFileUsage {
    id: [u8; UUID_LEN],
    size_bytes: u64,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_usage_index(u: LbUsage, i: usize) -> *mut LbFileUsage {
    u.usages.add(i)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_usage_free(r: LbUsage) {
    let _ = Vec::from_raw_parts(r.usages, r.num_usages, r.num_usages);
    lb_usage_item_metric_free(r.server_usage);
    lb_usage_item_metric_free(r.data_cap);
}

#[repr(C)]
pub struct LbUsageItemMetric {
    exact: u64,
    readable: *mut c_char,
}

fn lb_usage_item_metric_none() -> LbUsageItemMetric {
    LbUsageItemMetric { exact: 0, readable: null_mut() }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_usage_item_metric_free(m: LbUsageItemMetric) {
    libc::free(m.readable as *mut c_void);
}

#[repr(C)]
pub struct LbUsageResult {
    ok: LbUsage,
    err: LbError,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_usage_result_free(r: LbUsageResult) {
    if r.err.code == LbErrorCode::Success {
        lb_usage_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_usage_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_usage_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_usage(core: *mut c_void) -> LbUsageResult {
    let mut r = LbUsageResult {
        ok: LbUsage {
            usages: null_mut(),
            num_usages: 0,
            server_usage: lb_usage_item_metric_none(),
            data_cap: lb_usage_item_metric_none(),
        },
        err: lb_error_none(),
    };
    match core!(core).get_usage() {
        Ok(m) => {
            let mut usages = Vec::with_capacity(m.usages.len());
            for fu in m.usages {
                usages.push(LbFileUsage { id: fu.file_id.into_bytes(), size_bytes: fu.size_bytes });
            }
            let mut usages = std::mem::ManuallyDrop::new(usages);
            r.ok.usages = usages.as_mut_ptr();
            r.ok.num_usages = usages.len();
            r.ok.server_usage.exact = m.server_usage.exact;
            r.ok.server_usage.readable = cstr(m.server_usage.readable);
            r.ok.data_cap.exact = m.data_cap.exact;
            r.ok.data_cap.readable = cstr(m.data_cap.readable);
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

#[repr(C)]
pub struct LbUncUsageResult {
    ok: LbUsageItemMetric,
    err: LbError,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_unc_usage_result_free(r: LbUncUsageResult) {
    if r.err.code == LbErrorCode::Success {
        lb_usage_item_metric_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_unc_usage_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_usage_item_metric_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_uncompressed_usage(core: *mut c_void) -> LbUncUsageResult {
    let mut r = LbUncUsageResult { ok: lb_usage_item_metric_none(), err: lb_error_none() };
    match core!(core).get_uncompressed_usage() {
        Ok(im) => {
            r.ok.exact = im.exact;
            r.ok.readable = cstr(im.readable);
        }
        Err(err) => r.err = lberr(err),
    }
    r
}
