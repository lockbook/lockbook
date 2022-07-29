use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::document_repo;
use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use lockbook_shared::file_metadata::FileMetadataDiff;
use test_utils::assert_matches;
use test_utils::*;

#[test]
fn change_document_content() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();
    let mut doc = core.create_at_path("test.md").unwrap();
    let doc_enc = core.db.local_metadata.get(&doc.id).unwrap().unwrap();

    // create document
    api_service::request(
        &account,
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc_enc)] },
    )
    .unwrap();

    // get document metadata version
    doc.metadata_version = api_service::request(
        &account,
        GetUpdatesRequest { since_metadata_version: root.metadata_version },
    )
    .unwrap()
    .file_metadata[0]
        .metadata_version;

    core.write_document(doc.id, "new doc content".as_bytes())
        .unwrap();
    let new_content = document_repo::get(&core.config, RepoSource::Local, doc.id).unwrap();

    // change document content
    api_service::request(
        &account,
        ChangeDocRequest { id: doc.id, old_metadata_version: doc.metadata_version, new_content },
    )
    .unwrap();
}

#[test]
fn change_document_content_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap();
    core.write_document(doc.id, "content".as_bytes()).unwrap();
    let new_content = document_repo::get(&core.config, RepoSource::Local, doc.id).unwrap();

    // change content of document we never created
    let result = api_service::request(
        &account,
        ChangeDocRequest { id: doc.id, old_metadata_version: 0, new_content },
    );
    assert_matches!(
        result,
        Err(ApiError::<ChangeDocError>::Endpoint(ChangeDocError::DocumentNotFound))
    );
}
