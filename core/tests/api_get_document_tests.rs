use lockbook_core::service::api_service::{ApiError, Requester};
use lockbook_shared::api::*;
use lockbook_shared::crypto::AESEncrypted;

use lockbook_shared::file::like::FileLike;
use lockbook_shared::file::metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[test]
fn get_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    let old = core.db.base_metadata.get(&id).unwrap().unwrap();
    let mut new = old.clone();
    new.timestamped_value.value.document_hmac = Some([0; 32]);

    // update document content
    core.client
        .request(
            &account,
            ChangeDocRequest {
                diff: FileDiff::edit(&old, &new),
                new_content: AESEncrypted {
                    value: vec![69],
                    nonce: vec![69],
                    _t: Default::default(),
                },
            },
        )
        .unwrap();

    // get document
    let result = &core
        .client
        .request(&account, GetDocRequest { id, hmac: *new.document_hmac().unwrap() })
        .unwrap();
    assert_eq!(
        result.content,
        AESEncrypted { value: vec!(69), nonce: vec!(69), _t: Default::default() }
    );
}

#[test]
fn get_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    let mut old = core.db.base_metadata.get(&id).unwrap().unwrap();
    old.timestamped_value.value.id = Uuid::new_v4();
    let mut new = old;
    new.timestamped_value.value.document_hmac = Some([0; 32]);

    // get document we never created
    let result = core
        .client
        .request(&account, GetDocRequest { id: *new.id(), hmac: *new.document_hmac().unwrap() });
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
    );
}
