use std::ffi::{CString, c_char};

use lb_rs::Uuid;

use crate::ffi_utils::rvec;
use crate::lb_c_err::LbFfiErr;
use crate::lb_file::LbFile;
use crate::lb_work::LbSyncRes;
use crate::{
    LbAccountRes, LbDocRes, LbExportAccountQRRes, LbExportAccountRes, LbFileListRes, LbFileRes,
    LbIdListRes, LbInitRes, LbLastSyncedHuman, LbLastSyncedi64, LbPathRes, LbPathsRes, LbSearchRes,
    LbStatus, LbSubscriptionInfoRes, LbUncompressedRes, LbUsageMetricsRes,
};

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_str(str: *mut c_char) {
    unsafe { drop(CString::from_raw(str)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_err(err: *mut LbFfiErr) {
    if err.is_null() {
        return;
    }

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

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_init(init: LbInitRes) {
    if !init.err.is_null() {
        lb_free_err(init.err);
    }

    if !init.lb.is_null() {
        unsafe { drop(Box::from_raw(init.lb)) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_account(acc: LbAccountRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

    if !acc.username.is_null() {
        unsafe { drop(CString::from_raw(acc.username)) }
    }

    if !acc.api_url.is_null() {
        unsafe { drop(CString::from_raw(acc.api_url)) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_export_account(acc: LbExportAccountRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

    if !acc.account_string.is_null() {
        unsafe { drop(CString::from_raw(acc.account_string)) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_export_account_qr(acc: LbExportAccountQRRes) {
    if !acc.err.is_null() {
        lb_free_err(acc.err);
    }

    if !acc.qr.is_null() {
        drop(rvec(acc.qr, acc.qr_len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_path_res(path: LbPathRes) {
    if !path.err.is_null() {
        lb_free_err(path.err);
    }

    if !path.path.is_null() {
        unsafe { drop(CString::from_raw(path.path)) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_paths_res(paths: LbPathsRes) {
    if !paths.err.is_null() {
        lb_free_err(paths.err);
    }

    if !paths.paths.is_null() {
        let paths = rvec(paths.paths, paths.len);
        for path in &paths {
            unsafe { drop(CString::from_raw(*path)) };
        }

        drop(paths);
    }
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_file_res(file_res: LbFileRes) {
    if !file_res.err.is_null() {
        lb_free_err(file_res.err);
    }

    if !Uuid::from(file_res.file.id).is_nil() {
        lb_free_file(file_res.file);
    }
}

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_sync_res(sync_res: LbSyncRes) {
    if !sync_res.err.is_null() {
        lb_free_err(sync_res.err);
    }

    if !sync_res.work.work.is_null() {
        drop(rvec(sync_res.work.work, sync_res.work.len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_doc_res(doc: LbDocRes) {
    if !doc.err.is_null() {
        lb_free_err(doc.err);
    }

    if !doc.doc.is_null() {
        drop(rvec(doc.doc, doc.len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_last_synced_i64(last: LbLastSyncedi64) {
    if !last.err.is_null() {
        lb_free_err(last.err);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_last_synced_human(last: LbLastSyncedHuman) {
    if !last.err.is_null() {
        lb_free_err(last.err);
    }

    if !last.last.is_null() {
        unsafe { drop(CString::from_raw(last.last)) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_id_list_res(ids: LbIdListRes) {
    if !ids.err.is_null() {
        lb_free_err(ids.err);
    }

    if !ids.ids.is_null() {
        drop(rvec(ids.ids, ids.len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_usage_metrics(usage: LbUsageMetricsRes) {
    if !usage.err.is_null() {
        lb_free_err(usage.err);
    }

    if !usage.usages.server_cap_human.is_null() {
        unsafe { drop(CString::from_raw(usage.usages.server_cap_human)) }
    }

    if !usage.usages.server_used_human.is_null() {
        unsafe { drop(CString::from_raw(usage.usages.server_used_human)) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_uncompressed_usage(usage: LbUncompressedRes) {
    if !usage.err.is_null() {
        lb_free_err(usage.err);
    }

    if !usage.uncompressed_human.is_null() {
        unsafe { drop(CString::from_raw(usage.uncompressed_human)) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_search_results(search_results: LbSearchRes) {
    if !search_results.err.is_null() {
        lb_free_err(search_results.err);
    }

    if !search_results.results.is_null() {
        let results = rvec(search_results.results, search_results.results_len);

        for result in results {
            if !result.doc_result.is_null() {
                let result = unsafe { *Box::from_raw(result.doc_result) };

                let content_matches = rvec(result.content_matches, result.content_matches_len);

                for content_match in content_matches {
                    let _ =
                        rvec(content_match.matched_indicies, content_match.matched_indicies_len);

                    unsafe { drop(CString::from_raw(content_match.paragraph)) }
                }

                unsafe { drop(CString::from_raw(result.path)) }
            } else {
                let result = unsafe { *Box::from_raw(result.path_result) };

                let _ = rvec(result.matched_indicies, result.matched_indicies_len);
                unsafe { drop(CString::from_raw(result.path)) };
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_sub_info(sub_info: LbSubscriptionInfoRes) {
    if !sub_info.err.is_null() {
        lb_free_err(sub_info.err);
    }

    if !sub_info.info.is_null() {
        let sub_info = unsafe { Box::from_raw(sub_info.info) };

        if !sub_info.stripe.is_null() {
            let stripe = unsafe { Box::from_raw(sub_info.stripe) };

            unsafe { drop(CString::from_raw(stripe.card_last_4_digits)) }
        }

        if !sub_info.app_store.is_null() {
            unsafe { drop(Box::from_raw(sub_info.app_store)) };
        }

        if !sub_info.google_play.is_null() {
            unsafe { drop(Box::from_raw(sub_info.google_play)) };
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_free_status(status: LbStatus) {
    if !status.pushing_files.ids.is_null() {
        drop(rvec(status.pushing_files.ids, status.pushing_files.len));
    }

    if !status.pulling_files.ids.is_null() {
        drop(rvec(status.pulling_files.ids, status.pulling_files.len));
    }

    if !status.dirty_locally.ids.is_null() {
        drop(rvec(status.dirty_locally.ids, status.dirty_locally.len));
    }

    unsafe {
        if !status.space_used.is_null() {
            let usage = status.space_used;
            let usage = Box::from_raw(usage);

            if !usage.server_cap_human.is_null() {
                drop(CString::from_raw(usage.server_cap_human))
            }

            if !usage.server_used_human.is_null() {
                drop(CString::from_raw(usage.server_used_human))
            }

            drop(usage);
        }

        if !status.sync_status.is_null() {
            drop(CString::from_raw(status.sync_status));
        }
    }
}
