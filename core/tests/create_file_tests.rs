use lockbook_core::service::api_service;
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileMetadataDiff;
use test_utils::*;
use uuid::Uuid;

#[test]
fn create_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let id = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    )
    .unwrap();
}

#[test]
fn create_document_duplicate_id() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let id = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();
    core.sync(None).unwrap();

    // create document with same id and key
    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    );
    assert_matches!(result, UPDATES_REQ);
}

#[test]
fn create_document_duplicate_path() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // create document
    let id = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&id).unwrap().unwrap();
    core.sync(None).unwrap();

    // create document with same path
    doc.id = Uuid::new_v4();
    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    );

    assert_matches!(result, UPDATES_REQ);
}

#[test]
fn create_document_parent_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    // create document
    let id = core
        .create_at_path(&path(&core, "parent/test.md"))
        .unwrap()
        .id;
    let doc = core.db.local_metadata.get(&id).unwrap().unwrap();

    // create document
    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    );

    assert_matches!(result, UPDATES_REQ);
}
