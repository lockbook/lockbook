use lockbook_core::repo::schema::OneKey;
use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileDiff;
use test_utils::*;

#[test]
fn rename_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core.create_at_path("test.md").unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    api_service::request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] }).unwrap();

    let old = doc.clone();
    core.rename_file(*doc.id(), &random_name()).unwrap();
    let new = core.db.local_metadata.get(doc.id()).unwrap().unwrap();

    api_service::request(&account, UpsertRequest { updates: vec![FileDiff::edit(&old, &new)] })
        .unwrap();
}
