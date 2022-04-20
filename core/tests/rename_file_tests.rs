use lockbook_core::repo::schema::OneKey;
use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileMetadataDiff;
use lockbook_models::file_metadata::FileType;
use test_utils::*;

#[test]
fn rename_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    )
    .unwrap();

    let old_name = doc.name.clone();
    core.rename_file(doc.id, &random_name()).unwrap();
    let mut doc = core.db.local_metadata.get(&doc.id).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc)],
        },
    )
    .unwrap();
}

#[test]
fn rename_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            // create document as if renaming an existing document
            updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
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
fn rename_document_deleted() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    )
    .unwrap();

    let old_name = doc.name.clone();
    core.rename_file(doc.id, &random_name()).unwrap();
    let mut doc = core.db.local_metadata.get(&doc.id).unwrap().unwrap();
    doc.deleted = true;

    api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc)],
        },
    )
    .unwrap();
}

#[test]
fn rename_document_conflict() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core.create_at_path(&path(&core, "test.md")).unwrap().id;
    let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
    )
    .unwrap();

    core.rename_file(doc.id, &random_name()).unwrap();
    let mut doc = core.db.local_metadata.get(&doc.id).unwrap().unwrap();

    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            // use incorrect previous name
            updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
        },
    );
    assert_matches!(result, UPDATES_REQ);
}

#[test]
fn rename_document_path_taken() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    // create 2 document
    let doc1 = core.create_at_path(&path(&core, "test1.md")).unwrap().id;
    let mut doc1 = core.db.local_metadata.get(&doc1).unwrap().unwrap();

    let doc2 = core.create_at_path(&path(&core, "test2.md")).unwrap().id;
    let mut doc2 = core.db.local_metadata.get(&doc2).unwrap().unwrap();

    api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new(&doc1), FileMetadataDiff::new(&doc2)],
        },
    )
    .unwrap();

    // rename document to match name of other document
    let old_name = doc1.name.clone();
    doc1.name = doc2.name;

    let result = api_service::request(
        &account,
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc1)],
        },
    );
    assert_matches!(result, UPDATES_REQ);
}

#[test]
fn rename_folder_cannot_rename_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core
        .db
        .base_metadata
        .get(&core.db.root.get(&OneKey {}).unwrap().unwrap())
        .unwrap()
        .unwrap();

    let result = api_service::request(
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
