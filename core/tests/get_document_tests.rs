use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_models::api::*;
use lockbook_models::crypto::AESEncrypted;

use test_utils::*;
use uuid::Uuid;

#[test]
fn get_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    let metadata_version = core
        .db
        .base_metadata
        .get(&id)
        .unwrap()
        .unwrap()
        .metadata_version;

    // update document content
    api_service::request(
        &account,
        ChangeDocumentContentRequest {
            id,
            old_metadata_version: metadata_version,
            new_content: AESEncrypted { value: vec![69], nonce: vec![69], _t: Default::default() },
        },
    )
    .unwrap();

    // get content version
    let content_version = api_service::request(
        &account,
        GetUpdatesRequest { since_metadata_version: metadata_version },
    )
    .unwrap()
    .file_metadata
    .iter()
    .find(|&f| f.id == id)
    .unwrap()
    .content_version;

    // get document
    let result =
        &api_service::request(&account, GetDocumentRequest { id, content_version }).unwrap();
    assert_eq!(
        result.content,
        AESEncrypted { value: vec!(69), nonce: vec!(69), _t: Default::default() }
    );
}

#[test]
fn get_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // get document we never created
    let result = api_service::request(
        &account,
        GetDocumentRequest { id: Uuid::new_v4(), content_version: 0 },
    );
    assert_matches!(
        result,
        Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
    );
}
