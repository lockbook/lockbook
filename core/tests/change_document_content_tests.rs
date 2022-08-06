use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use lockbook_shared::crypto::AESEncrypted;
use lockbook_shared::file_metadata::FileDiff;
use test_utils::assert_matches;
use test_utils::*;
use uuid::Uuid;

#[test]
fn change_document_content() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    // create document
    api_service::request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] }).unwrap();

    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.document_hmac = Some([0; 32]);

    let diff = FileDiff::edit(&doc1, &doc2);
    // change document content
    api_service::request(
        &account,
        ChangeDocRequest {
            diff,
            new_content: AESEncrypted { value: vec![], nonce: vec![], _t: Default::default() },
        },
    )
    .unwrap();
}

#[test]
fn change_document_content_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    // create document
    api_service::request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] }).unwrap();

    doc.timestamped_value.value.id = Uuid::new_v4();
    let doc1 = doc;
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.document_hmac = Some([0; 32]);

    let diff = FileDiff::edit(&doc1, &doc2);
    // change document content
    let res = api_service::request(
        &account,
        ChangeDocRequest {
            diff,
            new_content: AESEncrypted { value: vec![], nonce: vec![], _t: Default::default() },
        },
    );
    assert_matches!(
        res,
        Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::DocumentNotFound))
    );
}
