use std::{
    ffi::{c_char, c_uchar},
    fs,
    path::PathBuf,
    process,
    ptr::null_mut,
};

use ffi_utils::{carray, cstring, lb_err, r_opt_str, r_paths, rlb, rstr, rstring, rvec};
use lb_c_err::LbFfiErr;
use lb_file::{LbFile, LbFileList, LbFileType};
pub use lb_rs::*;
pub use lb_rs::{blocking::Lb, model::core_config::Config};
use lb_rs::{model::file::ShareMode, service::activity::RankingWeights};
use lb_work::LbSyncRes;
use model::api::{
    AppStoreAccountState, GooglePlayAccountState, PaymentMethod, PaymentPlatform,
    StripeAccountTier, UnixTimeMillis,
};
use service::{
    import_export::ImportStatus,
    search::{SearchConfig, SearchResult},
};

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
            LbInitRes { lb, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbInitRes { lb: null_mut(), err }
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
            LbAccountRes { username, api_url, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbAccountRes { username: null_mut(), api_url: null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_import_account(
    lb: *mut Lb, key: *const c_char, api_url: *const c_char,
) -> LbAccountRes {
    let lb = rlb(lb);
    let key = rstr(key);
    let api_url = r_opt_str(api_url);

    match lb.import_account(key, api_url) {
        Ok(account) => {
            let username = cstring(account.username);
            let api_url = cstring(account.api_url);
            LbAccountRes { username, api_url, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbAccountRes { username: null_mut(), api_url: null_mut(), err }
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
            LbAccountRes { username, api_url, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbAccountRes { username: null_mut(), api_url: null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_delete_account(lb: *mut Lb) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.delete_account() {
        Ok(_) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn lb_logout_and_exit(lb: *mut Lb) {
    let lb = rlb(lb);
    fs::remove_dir_all(lb.get_config().writeable_path).unwrap();
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
            LbExportAccountRes { account_string, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbExportAccountRes { account_string: null_mut(), err }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_export_account_phrase(lb: *mut Lb) -> LbExportAccountRes {
    let lb = rlb(lb);

    match lb.export_account_phrase() {
        Ok(account_phrase) => {
            let account_string = cstring(account_phrase);
            LbExportAccountRes { account_string, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbExportAccountRes { account_string: null_mut(), err }
        }
    }
}

#[repr(C)]
pub struct LbExportAccountQRRes {
    err: *mut LbFfiErr,
    qr: *mut c_uchar,
    qr_len: usize,
}

#[no_mangle]
pub extern "C" fn lb_export_account_qr(lb: *mut Lb) -> LbExportAccountQRRes {
    let lb = rlb(lb);

    match lb.export_account_qr() {
        Ok(account_qr) => {
            let (qr, qr_len) = carray(account_qr);
            LbExportAccountQRRes { qr, qr_len, err: null_mut() }
        }
        Err(err) => {
            let err = lb_err(err);
            LbExportAccountQRRes { qr: null_mut(), qr_len: 0, err }
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
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => {
            let err = lb_err(err);
            LbFileRes { err, file: LbFile::default() }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_write_document(
    lb: *mut Lb, id: Uuid, ptr: *mut u8, len: usize,
) -> *mut LbFfiErr {
    let lb = rlb(lb);
    let data = rvec(ptr, len);
    match lb.write_document(id, &data) {
        Ok(()) => null_mut(),
        Err(e) => lb_err(e),
    }
}

#[no_mangle]
pub extern "C" fn lb_get_root(lb: *mut Lb) -> LbFileRes {
    let lb = rlb(lb);

    match lb.get_root() {
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => {
            let err = lb_err(err);
            LbFileRes { err, file: Default::default() }
        }
    }
}

#[repr(C)]
pub struct LbFileListRes {
    err: *mut LbFfiErr,
    list: LbFileList,
}

#[no_mangle]
pub extern "C" fn lb_get_children(lb: *mut Lb, id: Uuid) -> LbFileListRes {
    let lb = rlb(lb);
    match lb.get_children(&id) {
        Ok(children) => {
            let list = children.into();
            LbFileListRes { err: null_mut(), list }
        }
        Err(e) => LbFileListRes { err: lb_err(e), list: Default::default() },
    }
}

#[no_mangle]
pub extern "C" fn lb_get_and_get_children_recursively(lb: *mut Lb, id: Uuid) -> LbFileListRes {
    let lb = rlb(lb);
    match lb.get_and_get_children_recursively(&id) {
        Ok(children) => {
            let list = children.into();
            LbFileListRes { err: null_mut(), list }
        }
        Err(e) => LbFileListRes { err: lb_err(e), list: Default::default() },
    }
}

#[no_mangle]
pub extern "C" fn lb_get_file(lb: *mut Lb, id: Uuid) -> LbFileRes {
    let lb = rlb(lb);

    match lb.get_file_by_id(id) {
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => {
            let err = lb_err(err);
            LbFileRes { err, file: Default::default() }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_delete_file(lb: *mut Lb, id: Uuid) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.delete_file(&id) {
        Ok(_) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[repr(C)]
pub struct LbDocRes {
    err: *mut LbFfiErr,
    doc: *mut u8,
    len: usize,
}

#[no_mangle]
pub extern "C" fn lb_read_doc(lb: *mut Lb, id: Uuid) -> LbDocRes {
    let lb = rlb(lb);

    match lb.read_document(id) {
        Ok(doc) => {
            let (doc, len) = carray(doc);
            LbDocRes { err: null_mut(), doc, len }
        }
        Err(err) => {
            let err = lb_err(err);
            LbDocRes { err, doc: null_mut(), len: 0 }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_list_metadatas(lb: *mut Lb) -> LbFileListRes {
    let lb = rlb(lb);

    match lb.list_metadatas() {
        Ok(children) => {
            let list = children.into();
            LbFileListRes { err: null_mut(), list }
        }
        Err(e) => LbFileListRes { err: lb_err(e), list: Default::default() },
    }
}

#[no_mangle]
pub extern "C" fn lb_rename_file(lb: *mut Lb, id: Uuid, new_name: *const c_char) -> *mut LbFfiErr {
    let lb = rlb(lb);
    let new_name = rstr(new_name);

    match lb.rename_file(&id, new_name) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn lb_move_file(lb: *mut Lb, id: Uuid, new_parent: Uuid) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.move_file(&id, &new_parent) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn lb_share_file(
    lb: *mut Lb, id: Uuid, username: *const c_char, mode: ShareMode,
) -> *mut LbFfiErr {
    let lb = rlb(lb);
    let username = rstr(username);

    match lb.share_file(id, username, mode) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn lb_get_pending_shares(lb: *mut Lb) -> LbFileListRes {
    let lb = rlb(lb);

    match lb.get_pending_shares() {
        Ok(shares) => LbFileListRes { err: null_mut(), list: shares.into() },
        Err(err) => {
            let err = lb_err(err);
            LbFileListRes { err, list: LbFileList::default() }
        }
    }
}

#[no_mangle]
pub extern "C" fn lb_delete_pending_share(lb: *mut Lb, id: Uuid) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.delete_pending_share(&id) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn lb_create_link_at_path(
    lb: *mut Lb, path_and_name: *const c_char, target_id: Uuid,
) -> LbFileRes {
    let lb = rlb(lb);
    let path_and_name = rstr(path_and_name);

    match lb.create_link_at_path(path_and_name, target_id) {
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => LbFileRes { err: lb_err(err), file: Default::default() },
    }
}

#[no_mangle]
pub extern "C" fn lb_create_at_path(lb: *mut Lb, path_and_name: *const c_char) -> LbFileRes {
    let lb = rlb(lb);
    let path_and_name = rstr(path_and_name);

    match lb.create_at_path(path_and_name) {
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => LbFileRes { err: lb_err(err), file: Default::default() },
    }
}

#[no_mangle]
pub extern "C" fn lb_get_by_path(lb: *mut Lb, path: *const c_char) -> LbFileRes {
    let lb = rlb(lb);
    let path = rstr(path);

    match lb.get_by_path(path) {
        Ok(f) => LbFileRes { err: null_mut(), file: f.into() },
        Err(err) => LbFileRes { err: lb_err(err), file: Default::default() },
    }
}

#[repr(C)]
pub struct LbPathRes {
    err: *mut LbFfiErr,
    path: *const c_char,
}

#[no_mangle]
pub extern "C" fn lb_get_path_by_id(lb: *mut Lb, id: Uuid) -> LbPathRes {
    let lb = rlb(lb);

    match lb.get_path_by_id(id) {
        Ok(p) => LbPathRes { err: null_mut(), path: cstring(p) },
        Err(err) => LbPathRes { err: lb_err(err), path: null_mut() },
    }
}

#[repr(C)]
pub struct LbLocalChangesRes {
    err: *mut LbFfiErr,
    ids: *mut Uuid,
    len: usize,
}

#[no_mangle]
pub extern "C" fn lb_get_local_changes(lb: *mut Lb) -> LbLocalChangesRes {
    let lb = rlb(lb);

    match lb.get_local_changes() {
        Ok(ids) => {
            let (ids, len) = carray(ids);
            LbLocalChangesRes { err: null_mut(), ids, len }
        }
        Err(err) => LbLocalChangesRes { err: lb_err(err), ids: null_mut(), len: 0 },
    }
}

#[no_mangle]
pub extern "C" fn lb_debug_info(lb: *mut Lb, os_info: *const c_char) -> *const c_char {
    let lb = rlb(lb);
    let os_info = rstring(os_info);

    cstring(lb.debug_info(os_info))
}

#[no_mangle]
pub extern "C" fn lb_calculate_work(lb: *mut Lb) -> LbSyncRes {
    let lb = rlb(lb);
    lb.calculate_work().into()
}

#[no_mangle]
pub extern "C" fn lb_sync(lb: *mut Lb) -> LbSyncRes {
    let lb = rlb(lb);
    lb.sync(None).into()
}

#[repr(C)]
pub struct LbLastSyncedi64 {
    err: *mut LbFfiErr,
    last: i64,
}

#[repr(C)]
pub struct LbLastSyncedHuman {
    err: *mut LbFfiErr,
    last: *mut c_char,
}

#[no_mangle]
pub extern "C" fn lb_get_last_synced(lb: *mut Lb) -> LbLastSyncedi64 {
    let lb = rlb(lb);

    match lb.get_last_synced() {
        Ok(last) => LbLastSyncedi64 { err: null_mut(), last },
        Err(err) => LbLastSyncedi64 { err: lb_err(err), last: 0 },
    }
}

#[no_mangle]
pub extern "C" fn lb_get_last_synced_human_string(lb: *mut Lb) -> LbLastSyncedHuman {
    let lb = rlb(lb);

    match lb.get_last_synced_human_string() {
        Ok(last) => {
            let last = cstring(last);
            LbLastSyncedHuman { err: null_mut(), last }
        }
        Err(err) => LbLastSyncedHuman { err: lb_err(err), last: null_mut() },
    }
}

#[repr(C)]
pub struct LbIdListRes {
    err: *mut LbFfiErr,
    ids: *mut Uuid,
    len: usize,
}

#[no_mangle]
pub extern "C" fn lb_suggested_docs(lb: *mut Lb) -> LbIdListRes {
    let lb = rlb(lb);

    match lb.suggested_docs(RankingWeights::default()) {
        Ok(docs) => {
            let (ids, len) = carray(docs);
            LbIdListRes { err: null_mut(), ids, len }
        }
        Err(err) => LbIdListRes { err: lb_err(err), ids: null_mut(), len: 0 },
    }
}

#[repr(C)]
pub struct LbUsageMetricsRes {
    err: *mut LbFfiErr,
    usages: LbUsageMetrics,
}

#[repr(C)]
pub struct LbUsageMetrics {
    server_used_exact: u64,
    server_used_human: *mut c_char,

    server_cap_exact: u64,
    server_cap_human: *mut c_char,
}

#[no_mangle]
pub extern "C" fn get_usage(lb: *mut Lb) -> LbUsageMetricsRes {
    let lb = rlb(lb);

    match lb.get_usage() {
        Ok(usage) => LbUsageMetricsRes {
            err: null_mut(),
            usages: LbUsageMetrics {
                server_used_exact: usage.server_usage.exact,
                server_used_human: cstring(usage.server_usage.readable),
                server_cap_exact: usage.data_cap.exact,
                server_cap_human: cstring(usage.data_cap.readable),
            },
        },
        Err(err) => LbUsageMetricsRes {
            err: lb_err(err),
            usages: LbUsageMetrics {
                server_used_exact: 0,
                server_used_human: null_mut(),
                server_cap_exact: 0,
                server_cap_human: null_mut(),
            },
        },
    }
}

#[repr(C)]
pub struct LbUncompressedRes {
    err: *mut LbFfiErr,
    uncompressed_exact: u64,
    uncompressed_human: *mut c_char,
}

#[no_mangle]
pub extern "C" fn get_uncompressed_usage(lb: *mut Lb) -> LbUncompressedRes {
    let lb = rlb(lb);

    match lb.get_uncompressed_usage() {
        Ok(usage) => LbUncompressedRes {
            err: null_mut(),
            uncompressed_exact: usage.exact,
            uncompressed_human: cstring(usage.readable),
        },
        Err(err) => LbUncompressedRes {
            err: lb_err(err),
            uncompressed_exact: 0,
            uncompressed_human: null_mut(),
        },
    }
}

#[no_mangle]
pub extern "C" fn import_files(
    lb: *mut Lb, sources: *const *const c_char, sources_len: usize, dest: Uuid,
) -> *mut LbFfiErr {
    let lb = rlb(lb);

    let sources = r_paths(sources, sources_len);

    match lb.import_files(&sources, dest, &|_status: ImportStatus| println!("imported one file")) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn export_file(
    lb: *mut Lb, source_id: Uuid, dest: *const c_char, edit: bool,
) -> *mut LbFfiErr {
    let lb = rlb(lb);

    let dest = PathBuf::from(rstr(dest));

    match lb.export_files(source_id, dest, edit, &None) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[repr(C)]
pub struct LbSearchRes {
    err: *mut LbFfiErr,
    path_results: *mut LbPathSearchResult,
    path_results_len: usize,
    document_results: *mut LbDocumentSearchResult,
    document_results_len: usize,
}

#[repr(C)]
pub struct LbPathSearchResult {
    id: Uuid,
    path: *mut c_char,
    score: i64,
    matched_indicies: *mut usize,
    matched_indicies_len: usize,
}

#[repr(C)]
pub struct LbDocumentSearchResult {
    id: Uuid,
    path: *mut c_char,
    content_matches: *mut LbContentMatch,
    content_matches_len: usize,
}

#[repr(C)]
pub struct LbContentMatch {
    paragraph: *mut c_char,
    score: i64,
    matched_indicies: *mut usize,
    matched_indicies_len: usize,
}

#[no_mangle]
pub extern "C" fn search(
    lb: *mut Lb, input: *const c_char, search_paths: bool, search_docs: bool,
) -> LbSearchRes {
    let lb = rlb(lb);

    let input = rstr(input);
    let config = if search_paths && search_docs {
        SearchConfig::PathsAndDocuments
    } else if search_docs {
        SearchConfig::Documents
    } else {
        SearchConfig::Paths
    };

    match lb.search(input, config) {
        Ok(search_results) => {
            let mut path_results = Vec::new();
            let mut document_results = Vec::new();

            for result in search_results {
                match result {
                    SearchResult::PathMatch { id, path, matched_indices, score } => {
                        let (matched_indicies, matched_indicies_len) = carray(matched_indices);

                        path_results.push(LbPathSearchResult {
                            id,
                            path: cstring(path),
                            score,
                            matched_indicies,
                            matched_indicies_len,
                        });
                    }
                    SearchResult::DocumentMatch { id, path, content_matches } => {
                        let mut c_content_matches = Vec::new();

                        for content_match in content_matches {
                            let (matched_indicies, matched_indicies_len) =
                                carray(content_match.matched_indices);

                            c_content_matches.push(LbContentMatch {
                                paragraph: cstring(content_match.paragraph),
                                score: content_match.score,
                                matched_indicies,
                                matched_indicies_len,
                            });
                        }

                        let (content_matches, content_matches_len) = carray(c_content_matches);

                        document_results.push(LbDocumentSearchResult {
                            id,
                            path: cstring(path),
                            content_matches,
                            content_matches_len,
                        });
                    }
                }
            }

            let (path_results, path_results_len) =
                if path_results.is_empty() { (null_mut(), 0) } else { carray(path_results) };

            let (document_results, document_results_len) = if document_results.is_empty() {
                (null_mut(), 0)
            } else {
                carray(document_results)
            };

            LbSearchRes {
                err: null_mut(),
                path_results,
                path_results_len,
                document_results,
                document_results_len,
            }
        }
        Err(err) => LbSearchRes {
            err: lb_err(err),
            path_results: null_mut(),
            path_results_len: 0,
            document_results: null_mut(),
            document_results_len: 0,
        },
    }
}

#[no_mangle]
pub extern "C" fn upgrade_account_stripe(
    lb: *mut Lb, is_old_card: bool, number: *const c_char, exp_year: i32, exp_month: i32,
    cvc: *const c_char,
) -> *mut LbFfiErr {
    let lb = rlb(lb);

    let payment_method = if is_old_card {
        PaymentMethod::OldCard
    } else {
        PaymentMethod::NewCard { number: rstring(number), exp_year, exp_month, cvc: rstring(cvc) }
    };

    match lb.upgrade_account_stripe(StripeAccountTier::Premium(payment_method)) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn upgrade_account_app_store(
    lb: *mut Lb, original_transaction_id: *const c_char, app_account_token: *const c_char,
) -> *mut LbFfiErr {
    let lb = rlb(lb);

    let original_transaction_id = rstring(original_transaction_id);
    let app_account_token = rstring(app_account_token);

    match lb.upgrade_account_app_store(original_transaction_id, app_account_token) {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[no_mangle]
pub extern "C" fn cancel_subscription(lb: *mut Lb) -> *mut LbFfiErr {
    let lb = rlb(lb);

    match lb.cancel_subscription() {
        Ok(()) => null_mut(),
        Err(err) => lb_err(err),
    }
}

#[repr(C)]
pub struct LbSubscriptionInfoRes {
    err: *mut LbFfiErr,
    info: *mut LbSubscriptionInfo,
}

#[repr(C)]
pub struct LbSubscriptionInfo {
    period_end: UnixTimeMillis,
    stripe: *mut LbStripeSubscriptionInfo,
    app_store: *mut LbAppStoreSubscriptionInfo,
    google_play: *mut LbGooglePlaySubscriptionInfo,
}

#[repr(C)]
pub struct LbStripeSubscriptionInfo {
    card_last_4_digits: *mut c_char,
}

#[repr(C)]
pub struct LbGooglePlaySubscriptionInfo {
    is_state_ok: bool,
    is_state_canceled: bool,
    is_state_grace_period: bool,
    is_state_on_hold: bool,
}

#[repr(C)]
pub struct LbAppStoreSubscriptionInfo {
    is_state_ok: bool,
    is_state_grace_period: bool,
    is_state_failed_to_renew: bool,
    is_state_expired: bool,
}

#[no_mangle]
pub extern "C" fn get_subscription_info(lb: *mut Lb) -> LbSubscriptionInfoRes {
    let lb = rlb(lb);

    match lb.get_subscription_info() {
        Ok(info) => match info {
            Some(info) => {
                let (stripe, app_store, google_play) = match info.payment_platform {
                    PaymentPlatform::AppStore { account_state } => (
                        null_mut(),
                        Box::into_raw(Box::new(LbAppStoreSubscriptionInfo {
                            is_state_ok: account_state == AppStoreAccountState::Ok,
                            is_state_grace_period: account_state
                                == AppStoreAccountState::GracePeriod,
                            is_state_failed_to_renew: account_state
                                == AppStoreAccountState::FailedToRenew,
                            is_state_expired: account_state == AppStoreAccountState::Expired,
                        })),
                        null_mut(),
                    ),
                    PaymentPlatform::Stripe { card_last_4_digits } => (
                        Box::into_raw(Box::new(LbStripeSubscriptionInfo {
                            card_last_4_digits: cstring(card_last_4_digits),
                        })),
                        null_mut(),
                        null_mut(),
                    ),
                    PaymentPlatform::GooglePlay { account_state } => (
                        null_mut(),
                        null_mut(),
                        Box::into_raw(Box::new(LbGooglePlaySubscriptionInfo {
                            is_state_ok: account_state == GooglePlayAccountState::Ok,
                            is_state_canceled: account_state == GooglePlayAccountState::Canceled,
                            is_state_grace_period: account_state
                                == GooglePlayAccountState::GracePeriod,
                            is_state_on_hold: account_state == GooglePlayAccountState::OnHold,
                        })),
                    ),
                };

                let c_info = LbSubscriptionInfo {
                    period_end: info.period_end,
                    stripe,
                    app_store,
                    google_play,
                };

                LbSubscriptionInfoRes { err: null_mut(), info: Box::into_raw(Box::new(c_info)) }
            }
            None => LbSubscriptionInfoRes { err: null_mut(), info: null_mut() },
        },
        Err(err) => LbSubscriptionInfoRes { err: lb_err(err), info: null_mut() },
    }
}

// todo: pub fn get_subscription_info(&self) -> Result<Option<SubscriptionInfo>, LbError> {

mod ffi_utils;
mod lb_c_err;
mod lb_file;
mod lb_work;
mod mem_cleanup;
