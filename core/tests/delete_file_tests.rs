use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileMetadataDiff;
use test_utils::*;

#[test]
fn delete_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    core.sync(None).unwrap();
    let mut doc = core.db.base_metadata.get(&doc).unwrap().unwrap();
    let root = core.get_root().unwrap().id;
    // delete document
    doc.deleted = true;
    api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root, &doc.name, &doc)],
        },
    )
    .unwrap();
}

#[test]
fn delete_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let root = core.get_root().unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
    let mut diff = FileMetadataDiff::new_diff(root, &doc.name, &doc);
    diff.new_deleted = true;
    let result = api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            // create document as if deleting an existing document
            updates: vec![diff],
        },
    );
    assert_matches!(
        result,
        Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
            FileMetadataUpsertsError::NewFileHasOldParentAndName
        ))
    );
}

#[test]
fn delete_document_new_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
    doc.deleted = true;

    let result = api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            // create document as if deleting an existing document
            updates: vec![FileMetadataDiff::new(&doc)],
        },
    );
    assert_matches!(
        result,
        Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
            FileMetadataUpsertsError::NewFileDeleted
        ))
    );
}

#[test]
fn delete_document_deleted() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap().id;

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
    core.sync(None).unwrap();

    // delete document
    doc.deleted = true;
    api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root, &doc.name, &doc)],
        },
    )
    .unwrap();

    // delete document again
    api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root, &doc.name, &doc)],
        },
    )
    .unwrap();
}

#[test]
fn delete_cannot_delete_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap().id;
    let mut root = core.db.base_metadata.get(&root).unwrap().unwrap();

    root.deleted = true;
    let result = api_service::request(
        &core.client,
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root.id, &root.name, &root)],
        },
    );
    assert_matches!(
        result,
        Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
            FileMetadataUpsertsError::RootImmutable
        ))
    );
}
