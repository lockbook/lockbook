use lb_rs::service::api_service::{ApiError, Requester};
use lockbook_shared::api::*;
use lockbook_shared::file_metadata::FileDiff;
use test_utils::*;
use uuid::Uuid;

#[test]
fn delete_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    let doc1 = core
        .in_tx(|s| Ok(s.db.base_metadata.get().get(&doc).unwrap().clone()))
        .unwrap(); // delete document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.is_deleted = true;
    core.in_tx(|s| {
        s.client
            .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] })
            .unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_document_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("test.md").unwrap().id;
    core.sync(None).unwrap();
    let mut doc1 = core
        .in_tx(|s| Ok(s.db.base_metadata.get().get(&doc).unwrap().clone()))
        .unwrap();
    doc1.timestamped_value.value.id = Uuid::new_v4();
    // delete document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.is_deleted = true;
    core.in_tx(|s| {
        let result = s.client.request(
            &account,
            UpsertRequest {
                // create document as if deleting an existing document
                updates: vec![FileDiff::edit(&doc1, &doc2)],
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<UpsertError>::Endpoint(UpsertError::OldFileNotFound))
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_document_new_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("test.md").unwrap().id;
    let mut doc = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&doc).unwrap().clone()))
        .unwrap();
    doc.timestamped_value.value.is_deleted = true;

    core.in_tx(|s| {
        let result = s
            .client
            .request(&account, UpsertRequest { updates: vec![FileDiff::new(&doc)] });
        assert_matches!(result, Ok(_));
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_document_deleted() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("test.md").unwrap().id;
    let doc = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&doc).unwrap().clone()))
        .unwrap();
    core.sync(None).unwrap();

    // delete document
    let mut doc2 = doc.clone();
    doc2.timestamped_value.value.is_deleted = true;
    core.in_tx(|s| {
        s.client
            .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc, &doc2)] })
            .unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_cannot_delete_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap().id;
    let root1 = core
        .in_tx(|s| Ok(s.db.base_metadata.get().get(&root).unwrap().clone()))
        .unwrap();

    let mut root2 = root1.clone();
    root2.timestamped_value.value.is_deleted = true;
    core.in_tx(|s| {
        let result = s
            .client
            .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&root1, &root2)] });
        assert_matches!(
            result,
            Err(ApiError::<UpsertError>::Endpoint(UpsertError::RootModificationInvalid))
        );
        Ok(())
    })
    .unwrap();
}
