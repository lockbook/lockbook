use crate::*;

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
    let content = if len == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(content, len) }.to_vec()
    };
    let hmac = version.present.then_some(version.hmac);
    match rlb(lb).safe_write(id.into(), hmac, content, None) {
        Ok(hmac) => LbDocumentVersionRes { err: null_mut(), version: Some(hmac).into() },
        Err(err) => LbDocumentVersionRes { err: lb_err(err), version: Default::default() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lb_c_err::LbEC;
    use crate::mem_cleanup::{lb_free_doc_res, lb_free_err};

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
}
