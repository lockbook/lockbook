use lockbook_core::service::api_service::{ApiError, Requester};
use lockbook_shared::file_metadata::FileDiff;
use lockbook_shared::{api::*, ValidationFailure};
use test_utils::*;
use uuid::Uuid;

#[test]
fn create_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").unwrap().id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();

    core.client
        .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] })
        .unwrap();
}

#[test]
fn create_document_duplicate_id() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let id = core.create_at_path("test.md").unwrap().id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();
    core.sync(None).unwrap();

    // create document with same id and key
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] });

    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldVersionRequired))
    );
}

#[test]
fn create_document_duplicate_path() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // create document
    let id = core.create_at_path("test.md").unwrap().id;
    let mut doc = core.db.local_metadata.get(&id).unwrap().unwrap();
    core.sync(None).unwrap();

    // create document with same path

    doc.timestamped_value.value.id = Uuid::new_v4();
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] });

    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(
            ValidationFailure::PathConflict(_)
        )))
    );
}

#[test]
fn create_document_parent_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    // create document
    let id = core.create_at_path("parent/test.md").unwrap().id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();

    // create document
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] });

    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(ValidationFailure::Orphan(
            _
        ))))
    );
}
