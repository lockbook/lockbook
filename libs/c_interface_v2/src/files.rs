use std::path::PathBuf;

use lockbook_core::{File, FileType, ImportStatus, Share, ShareMode, SupportedImageFormats, Uuid};

use crate::*;

pub const UUID_LEN: usize = 16;

#[repr(C)]
pub struct LbFileId {
    data: [u8; UUID_LEN],
}

impl From<LbFileId> for Uuid {
    fn from(v: LbFileId) -> Uuid {
        Uuid::from_bytes(v.data)
    }
}

#[repr(C)]
pub struct LbFile {
    id: [u8; UUID_LEN],
    parent: [u8; UUID_LEN],
    name: *mut c_char,
    typ: LbFileType,
    lastmod_by: *mut c_char,
    lastmod: u64,
    shares: LbShareList,
}

pub unsafe fn lb_file_new(f: File) -> LbFile {
    let mut typ = lb_file_type_doc();
    if let FileType::Folder = f.file_type {
        typ.tag = LbFileTypeTag::Folder;
    }
    if let FileType::Link { target } = f.file_type {
        typ.tag = LbFileTypeTag::Link;
        typ.link_target = target.into_bytes();
    }
    LbFile {
        id: f.id.into_bytes(),
        parent: f.parent.into_bytes(),
        name: cstr(f.name),
        typ,
        lastmod_by: cstr(f.last_modified_by),
        lastmod: f.last_modified,
        shares: lb_share_list_new(f.shares),
    }
}

#[no_mangle]
pub unsafe extern "C" fn lb_file_free(f: LbFile) {
    let mut f = f;
    lb_file_free_ptr(&mut f as *mut LbFile);
}

pub unsafe fn lb_file_free_ptr(f: *mut LbFile) {
    libc::free((*f).name as *mut c_void);
    libc::free((*f).lastmod_by as *mut c_void);
    lb_share_list_free(&(*f).shares);
}

/// The zero value represents a document.
#[repr(C)]
pub struct LbFileType {
    tag: LbFileTypeTag,
    link_target: [u8; 16],
}

#[repr(C)]
pub enum LbFileTypeTag {
    Document,
    Folder,
    Link,
}

#[no_mangle]
pub extern "C" fn lb_file_type_doc() -> LbFileType {
    LbFileType {
        tag: LbFileTypeTag::Document,
        link_target: [0; 16],
    }
}

#[repr(C)]
pub struct LbShareList {
    list: *mut LbShare,
    count: usize,
}

unsafe fn lb_share_list_new(shares: Vec<Share>) -> LbShareList {
    let mut list = Vec::with_capacity(shares.len());
    for sh in shares {
        list.push(LbShare {
            by: cstr(sh.shared_by),
            with: cstr(sh.shared_with),
            mode: match sh.mode {
                ShareMode::Read => LbShareMode::Read,
                ShareMode::Write => LbShareMode::Write,
            },
        });
    }
    let mut list = std::mem::ManuallyDrop::new(list);
    LbShareList {
        list: list.as_mut_ptr(),
        count: list.len(),
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_share_list_index(sl: LbShareList, i: usize) -> *mut LbShare {
    sl.list.add(i)
}

unsafe fn lb_share_list_free(sl: &LbShareList) {
    let list = Vec::from_raw_parts(sl.list, sl.count, sl.count);
    for sh in list {
        libc::free(sh.by as *mut c_void);
        libc::free(sh.with as *mut c_void);
    }
}

#[repr(C)]
pub struct LbShare {
    by: *mut c_char,
    with: *mut c_char,
    mode: LbShareMode,
}

#[repr(C)]
pub enum LbShareMode {
    Read,
    Write,
}

#[repr(C)]
pub struct LbFileResult {
    ok: LbFile,
    err: LbError,
}

fn lb_file_result_new() -> LbFileResult {
    LbFileResult {
        ok: LbFile {
            id: [0; 16],
            parent: [0; 16],
            name: null_mut(),
            typ: lb_file_type_doc(),
            lastmod_by: null_mut(),
            lastmod: 0,
            shares: LbShareList {
                list: std::ptr::null_mut(),
                count: 0,
            },
        },
        err: lb_error_none(),
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_file_result_free(r: LbFileResult) {
    if r.err.code == LbErrorCode::Success {
        lb_file_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

#[repr(C)]
pub struct LbFileListResult {
    ok: LbFileList,
    err: LbError,
}

fn lb_file_list_result_new() -> LbFileListResult {
    LbFileListResult {
        ok: LbFileList {
            list: null_mut(),
            count: 0,
        },
        err: lb_error_none(),
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_file_list_result_free(r: LbFileListResult) {
    if r.err.code == LbErrorCode::Success {
        lb_file_list_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

#[repr(C)]
pub struct LbFileList {
    list: *mut LbFile,
    count: usize,
}

unsafe fn lb_file_list_init(fl: &mut LbFileList, from: Vec<File>) {
    let mut into = Vec::with_capacity(from.len());
    for f in from {
        into.push(lb_file_new(f));
    }
    let mut into = std::mem::ManuallyDrop::new(into);
    fl.list = into.as_mut_ptr();
    fl.count = into.len();
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_file_list_index(fl: LbFileList, i: usize) -> *mut LbFile {
    fl.list.add(i)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_file_list_free(fl: LbFileList) {
    let list = Vec::from_raw_parts(fl.list, fl.count, fl.count);
    for f in list {
        lb_file_free(f);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_file_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_create_file(
    core: *mut c_void,
    name: *const c_char,
    parent: LbFileId,
    ft: LbFileType,
) -> LbFileResult {
    let mut r = lb_file_result_new();
    let ftype = match ft.tag {
        LbFileTypeTag::Document => FileType::Document,
        LbFileTypeTag::Folder => FileType::Folder,
        LbFileTypeTag::Link => FileType::Link {
            target: Uuid::from_bytes(ft.link_target),
        },
    };
    match core!(core).create_file(rstr(name), parent.into(), ftype) {
        Ok(f) => r.ok = lb_file_new(f),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_create_file_at_path(
    core: *mut c_void,
    path_and_name: *const c_char,
) -> LbFileResult {
    let mut r = lb_file_result_new();
    match core!(core).create_at_path(rstr(path_and_name)) {
        Ok(f) => r.ok = lb_file_new(f),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_file_by_id(core: *mut c_void, id: LbFileId) -> LbFileResult {
    let mut r = lb_file_result_new();
    match core!(core).get_file_by_id(id.into()) {
        Ok(f) => r.ok = lb_file_new(f),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_file_by_path(
    core: *mut c_void,
    path: *const c_char,
) -> LbFileResult {
    let mut r = lb_file_result_new();
    match core!(core).get_by_path(rstr(path)) {
        Ok(f) => r.ok = lb_file_new(f),
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
pub unsafe extern "C" fn lb_get_path_by_id(core: *mut c_void, id: LbFileId) -> LbStringResult {
    let mut r = lb_string_result_new();
    match core!(core).get_path_by_id(id.into()) {
        Ok(path) => r.ok = cstr(path),
        Err(err) => r.err = lberr_unexpected(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_free` or `lb_error_free`
/// respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_root(core: *mut c_void) -> LbFileResult {
    let mut r = lb_file_result_new();
    match core!(core).get_root() {
        Ok(f) => r.ok = lb_file_new(f),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_list_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_list_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_children(core: *mut c_void, id: LbFileId) -> LbFileListResult {
    let mut r = lb_file_list_result_new();
    match core!(core).get_children(id.into()) {
        Ok(files) => lb_file_list_init(&mut r.ok, files),
        Err(err) => r.err = lberr_unexpected(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_list_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_list_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_and_get_children_recursively(
    core: *mut c_void,
    id: LbFileId,
) -> LbFileListResult {
    let mut r = lb_file_list_result_new();
    match core!(core).get_and_get_children_recursively(id.into()) {
        Ok(files) => lb_file_list_init(&mut r.ok, files),
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_file_list_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_list_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_list_metadatas(core: *mut c_void) -> LbFileListResult {
    let mut r = lb_file_list_result_new();
    match core!(core).list_metadatas() {
        Ok(files) => lb_file_list_init(&mut r.ok, files),
        Err(err) => r.err = lberr_unexpected(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_bytes_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_bytes_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_read_document(core: *mut c_void, id: LbFileId) -> LbBytesResult {
    let mut r = lb_bytes_result_new();
    match core!(core).read_document(id.into()) {
        Ok(mut data) => {
            data.shrink_to_fit();
            let mut data = std::mem::ManuallyDrop::new(data);
            r.ok.data = data.as_mut_ptr();
            r.ok.size = data.len();
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_write_document(
    core: *mut c_void,
    id: LbFileId,
    data: *const u8,
    len: i32,
) -> LbError {
    let data = std::slice::from_raw_parts(data, len as usize);
    match core!(core).write_document(id.into(), data) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

/// The non-zero value of each field determines which type of update this is. So if `total` is
/// non-zero, this is a "total calculated" update. If there's a `disk_path`, this is a "file
/// started" update. If there's a `file_done`, this is a "file finished" update.
#[repr(C)]
pub struct LbImportFileInfo {
    pub total: usize,
    pub disk_path: *mut c_char,
    pub file_done: *mut LbFile,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_import_file_info_free(fi: LbImportFileInfo) {
    if !fi.disk_path.is_null() {
        libc::free(fi.disk_path as *mut c_void);
    }
    if !fi.file_done.is_null() {
        lb_file_free_ptr(fi.file_done);
    }
}

pub type LbImportFileCallback = unsafe extern "C" fn(LbImportFileInfo, *mut c_void);

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_import_file(
    core: *mut c_void,
    disk_path: *mut c_char,
    dest: LbFileId,
    progress: LbImportFileCallback,
    user_data: *mut c_void,
) -> LbError {
    let src = PathBuf::from(rstr(disk_path));
    let func = move |info| {
        let mut c_info = LbImportFileInfo {
            total: 0,
            disk_path: null_mut(),
            file_done: null_mut(),
        };
        match info {
            ImportStatus::CalculatedTotal(total) => c_info.total = total,
            ImportStatus::StartingItem(p) => c_info.disk_path = cstr(p),
            ImportStatus::FinishedItem(f) => c_info.file_done = &mut lb_file_new(f),
        }
        progress(c_info, user_data);
    };
    match core!(core).import_files(&[src], dest.into(), &func) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

#[repr(C)]
pub struct LbExportFileInfo {
    pub disk_path: *mut c_char,
    pub lb_path: *mut c_char,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_export_file_info_free(fi: LbExportFileInfo) {
    libc::free(fi.disk_path as *mut c_void);
    libc::free(fi.lb_path as *mut c_void);
}

pub type LbExportFileCallback = unsafe extern "C" fn(LbExportFileInfo, *mut c_void);

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_export_file(
    core: *mut c_void,
    id: LbFileId,
    dest: *const c_char,
    progress: LbExportFileCallback,
    user_data: *mut c_void,
) -> LbError {
    let mut e = lb_error_none();
    if let Err(err) = core!(core).export_file(
        id.into(),
        rstr(dest).into(),
        false,
        Some(Box::new(move |info| {
            let c_info = LbExportFileInfo {
                disk_path: cstr(info.disk_path.to_string_lossy().to_string()),
                lb_path: cstr(info.lockbook_path),
            };
            progress(c_info, user_data)
        })),
    ) {
        e = lberr(err);
    }
    e
}

/// # Safety
///
/// The returned value must be passed to `lb_bytes_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_bytes_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_export_drawing(
    core: *mut c_void,
    id: LbFileId,
    fmt_code: u8,
) -> LbBytesResult {
    let mut r = lb_bytes_result_new();
    // These values are bound together in a unit test in this crate.
    let img_fmt = match get_img_fmt_code(fmt_code) {
        Ok(v) => v,
        Err(err) => {
            r.err = err;
            return r;
        }
    };
    match core!(core).export_drawing(id.into(), img_fmt, None) {
        Ok(mut data) => {
            data.shrink_to_fit();
            let mut data = std::mem::ManuallyDrop::new(data);
            r.ok.data = data.as_mut_ptr();
            r.ok.size = data.len();
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_export_drawing_to_disk(
    core: *mut c_void,
    id: LbFileId,
    fmt_code: u8,
    dest: *mut c_char,
) -> LbError {
    // These values are bound together in a unit test in this crate.
    let img_fmt = match get_img_fmt_code(fmt_code) {
        Ok(v) => v,
        Err(err) => return err,
    };
    let location = rstr(dest);
    match core!(core).export_drawing_to_disk(id.into(), img_fmt, None, location) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

unsafe fn get_img_fmt_code(n: u8) -> Result<SupportedImageFormats, LbError> {
    match n {
        0 => Ok(SupportedImageFormats::Png),
        1 => Ok(SupportedImageFormats::Jpeg),
        2 => Ok(SupportedImageFormats::Pnm),
        3 => Ok(SupportedImageFormats::Tga),
        4 => Ok(SupportedImageFormats::Farbfeld),
        5 => Ok(SupportedImageFormats::Bmp),
        n => Err(LbError {
            msg: cstr(format!("unknown image format code {}", n)),
            code: LbErrorCode::Unexpected,
            trace: null_mut(),
        }),
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_delete_file(core: *mut c_void, id: LbFileId) -> LbError {
    match core!(core).delete_file(id.into()) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_move_file(
    core: *mut c_void,
    id: LbFileId,
    new_parent: LbFileId,
) -> LbError {
    match core!(core).move_file(id.into(), new_parent.into()) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_rename_file(
    core: *mut c_void,
    id: LbFileId,
    new_name: *const c_char,
) -> LbError {
    match core!(core).rename_file(id.into(), rstr(new_name)) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_share_file(
    core: *mut c_void,
    id: LbFileId,
    uname: *const c_char,
    mode: LbShareMode,
) -> LbError {
    let mut e = lb_error_none();
    let mode = match mode {
        LbShareMode::Read => ShareMode::Read,
        LbShareMode::Write => ShareMode::Write,
    };
    if let Err(err) = core!(core).share_file(id.into(), rstr(uname), mode) {
        e = lberr(err);
    }
    e
}

/// # Safety
///
/// The returned value must be passed to `lb_file_list_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_file_list_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_pending_shares(core: *mut c_void) -> LbFileListResult {
    let mut r = lb_file_list_result_new();
    match core!(core).get_pending_shares() {
        Ok(files) => lb_file_list_init(&mut r.ok, files),
        Err(err) => r.err = lberr_unexpected(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_delete_pending_share(core: *mut c_void, id: LbFileId) -> LbError {
    match core!(core).delete_pending_share(id.into()) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}
