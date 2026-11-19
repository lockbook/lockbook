use crate::*;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::filename::DocumentType;

/// An opaque content version. `present == false` represents a new, empty document.
#[repr(C)]
#[derive(Default)]
pub struct LbDocumentVersion {
    pub present: bool,
    pub hmac: [u8; 32],
}

impl From<Option<model::file_metadata::DocumentHmac>> for LbDocumentVersion {
    fn from(hmac: Option<model::file_metadata::DocumentHmac>) -> Self {
        Self { present: hmac.is_some(), hmac: hmac.unwrap_or_default() }
    }
}

#[repr(C)]
pub struct LbDocumentVersionRes {
    pub err: *mut LbFfiErr,
    pub version: LbDocumentVersion,
}

/// Read a content version without downloading the document. Free `err` with `lb_free_err`.
#[unsafe(no_mangle)]
pub extern "C" fn lb_document_version(lb: *mut Lb, id: LbUuid) -> LbDocumentVersionRes {
    match rlb(lb).document_hmac(id.into()) {
        Ok(version) => LbDocumentVersionRes { err: null_mut(), version: version.into() },
        Err(err) => LbDocumentVersionRes { err: lb_err(err), version: Default::default() },
    }
}

/// Read the bytes and their matching version together. Free the result with `lb_free_doc_res`.
/// `version` must point to writable storage for one `LbDocumentVersion`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lb_read_document_versioned(
    lb: *mut Lb, id: LbUuid, version: *mut LbDocumentVersion,
) -> LbDocRes {
    match rlb(lb).read_document_with_hmac(id.into(), false) {
        Ok((hmac, doc)) => {
            version.write(hmac.into());
            let (doc, len) = carray(doc);
            LbDocRes { err: null_mut(), doc, len }
        }
        Err(err) => LbDocRes { err: lb_err(err), doc: null_mut(), len: 0 },
    }
}

/// Reject a stale save with `ReReadRequired`, including writes from other processes.
/// Free `err` with `lb_free_err`.
#[unsafe(no_mangle)]
pub extern "C" fn lb_write_document_versioned(
    lb: *mut Lb, id: LbUuid, version: LbDocumentVersion, content: *const u8, len: usize,
) -> LbDocumentVersionRes {
    let content = if len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(content, len) } };
    let hmac = version.present.then_some(version.hmac);
    match rlb(lb).safe_write(id.into(), hmac, content, None) {
        Ok(hmac) => LbDocumentVersionRes { err: null_mut(), version: Some(hmac).into() },
        Err(err) => LbDocumentVersionRes { err: lb_err(err), version: Default::default() },
    }
}

#[repr(C)]
pub struct LbWriteFromBaseRes {
    pub err: *mut LbFfiErr,
    pub version: LbDocumentVersion,
    pub merged: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_write_document_from_base(
    lb: *mut Lb, id: LbUuid, base: *const u8, base_len: usize, content: *const u8, len: usize,
) -> LbWriteFromBaseRes {
    let bytes = |ptr: *const u8, len: usize| {
        if len == 0 { vec![] } else { unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec() }
    };
    let base = bytes(base, base_len);
    match rlb(lb).write_from_base(id.into(), &base, bytes(content, len), None) {
        Ok((hmac, merged)) => LbWriteFromBaseRes { err: null_mut(), version: hmac.into(), merged },
        Err(err) => {
            LbWriteFromBaseRes { err: lb_err(err), version: Default::default(), merged: false }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_document_mergeable(name: *const c_char) -> bool {
    DocumentType::from_file_name_using_extension(rstr(name)) != DocumentType::Other
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_save_copy_beside(
    lb: *mut Lb, id: LbUuid, content: *const u8, len: usize,
) -> LbFileRes {
    let lb = rlb(lb);
    let content = if len == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(content, len) }.to_vec()
    };
    let copy = || -> LbResult<File> {
        let original = lb.get_file_by_id(id.into())?;
        let siblings = lb.get_children(&original.parent)?;
        let name = NameComponents::from(&original.name).free_copy_in(&siblings);
        let copy = lb.create_file(&name, &original.parent, FileType::Document)?;
        lb.write_document(copy.id, &content)?;
        Ok(copy)
    };
    match copy() {
        Ok(file) => LbFileRes { err: null_mut(), file: file.into() },
        Err(err) => LbFileRes { err: lb_err(err), file: Default::default() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lb_c_err::LbEC;
    use crate::mem_cleanup::{lb_free_doc_res, lb_free_err};
    use std::ffi::CString;

    #[test]
    fn versioned_reads_and_writes_reject_another_instances_edit() {
        let config = test_utils::test_config();
        let mut core = Lb::init(config.clone()).unwrap();
        core.create_account(&test_utils::random_name(), &test_utils::url(), false)
            .unwrap();
        let id = core.create_at_path("test.md").unwrap().id;
        let peer = Lb::init(config).unwrap();
        let initial = lb_document_version(&mut core, id.into());
        assert!(initial.err.is_null());
        assert!(!initial.version.present);

        let first = lb_write_document_versioned(
            &mut core,
            id.into(),
            initial.version,
            b"first".as_ptr(),
            5,
        );
        assert!(first.err.is_null());
        let mut version = LbDocumentVersion::default();
        let read = unsafe { lb_read_document_versioned(&mut core, id.into(), &mut version) };
        assert!(read.err.is_null());
        assert_eq!(unsafe { std::slice::from_raw_parts(read.doc, read.len) }, b"first");
        assert_eq!(version.hmac, first.version.hmac);
        lb_free_doc_res(read);

        peer.write_document(id, b"newer").unwrap();
        let stale =
            lb_write_document_versioned(&mut core, id.into(), version, b"stale".as_ptr(), 5);
        assert!(!stale.err.is_null());
        assert!(matches!(unsafe { &(*stale.err).code }, LbEC::ReReadRequired));
        lb_free_err(stale.err);
        assert_eq!(core.read_document(id, false).unwrap(), b"newer");

        let latest = lb_document_version(&mut core, id.into());
        let empty =
            lb_write_document_versioned(&mut core, id.into(), latest.version, std::ptr::null(), 0);
        assert!(empty.err.is_null());
        assert!(core.read_document(id, false).unwrap().is_empty());
    }

    #[test]
    fn writes_from_a_stale_base_merge_text_and_refuse_other_files() {
        let config = test_utils::test_config();
        let mut core = Lb::init(config.clone()).unwrap();
        core.create_account(&test_utils::random_name(), &test_utils::url(), false)
            .unwrap();
        let base = b"one\ntwo\nthree\n";
        let note = core.create_at_path("note.md").unwrap().id;
        let unchanged = core.create_at_path("plain.md").unwrap().id;
        let image = core.create_at_path("image.png").unwrap().id;
        for id in [note, unchanged, image] {
            core.write_document(id, base).unwrap();
        }
        let peer = Lb::init(config).unwrap();

        peer.write_document(note, b"one\ntwo\nthree\nfrom lockbook\n")
            .unwrap();
        let ours = b"from finder\none\ntwo\nthree\n";
        let merged = lb_write_document_from_base(
            &mut core,
            note.into(),
            base.as_ptr(),
            base.len(),
            ours.as_ptr(),
            ours.len(),
        );
        assert!(merged.err.is_null());
        assert!(merged.merged);
        let result = core.read_document(note, false).unwrap();
        assert_eq!(result, b"from finder\none\ntwo\nthree\nfrom lockbook\n");
        assert_eq!(lb_document_version(&mut core, note.into()).version.hmac, merged.version.hmac);

        let written = lb_write_document_from_base(
            &mut core,
            unchanged.into(),
            base.as_ptr(),
            base.len(),
            ours.as_ptr(),
            ours.len(),
        );
        assert!(written.err.is_null());
        assert!(!written.merged);
        assert_eq!(core.read_document(unchanged, false).unwrap(), ours);

        peer.write_document(image, b"theirs").unwrap();
        let refused = lb_write_document_from_base(
            &mut core,
            image.into(),
            base.as_ptr(),
            base.len(),
            ours.as_ptr(),
            ours.len(),
        );
        assert!(matches!(unsafe { &(*refused.err).code }, LbEC::ReReadRequired));
        lb_free_err(refused.err);
        assert_eq!(core.read_document(image, false).unwrap(), b"theirs");

        let copy = lb_save_copy_beside(&mut core, image.into(), ours.as_ptr(), ours.len());
        assert!(copy.err.is_null());
        let copy = core.get_file_by_id(copy.file.id.into()).unwrap();
        assert_eq!(copy.name, "image copy.png");
        assert_eq!(core.read_document(copy.id, false).unwrap(), ours);

        let mergeable = |name: &str| lb_document_mergeable(CString::new(name).unwrap().as_ptr());
        assert!(mergeable("note.md") && mergeable("drawing.svg") && mergeable("data.json"));
        assert!(!mergeable("image.png") && !mergeable("notes"));
    }
}
