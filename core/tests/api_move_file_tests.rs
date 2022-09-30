use lockbook_core::service::api_service::{ApiError, Requester};
use test_utils::*;
use uuid::Uuid;

use lockbook_shared::api::*;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileDiff;
use lockbook_shared::ValidationFailure;

#[test]
fn move_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("doc.md").unwrap().id;
    let folder = core.create_at_path("folder/").unwrap().id;
    core.sync(None).unwrap();

    let doc1 = core.db.base_metadata.get(&doc).unwrap().unwrap();

    // move document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.parent = folder;
    core.client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] })
        .unwrap();
}

#[test]
fn move_document_parent_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // create document and folder, but don't send folder to server
    let doc = core.create_at_path("folder/doc.md").unwrap().id;
    core.sync(None).unwrap();
    let doc1 = core.db.base_metadata.get(&doc).unwrap().unwrap();

    // move document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.parent = Uuid::new_v4();

    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] });
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[test]
fn move_document_deleted() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let doc = core.create_at_path("doc.md").unwrap().id;
    let doc1 = core.db.local_metadata.get(&doc).unwrap().unwrap();

    let folder = core.create_at_path("folder/").unwrap().id;
    let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();

    core.client
        .request(
            &account,
            UpsertRequest { updates: vec![FileDiff::new(&doc1), FileDiff::new(&folder)] },
        )
        .unwrap();

    // move & delete document
    let mut doc2 = doc1.clone();
    doc2.timestamped_value.value.is_deleted = true;
    doc2.timestamped_value.value.parent = *folder.id();
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] });
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::DeletedFileUpdated))
    );
}

#[test]
fn move_document_path_taken() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let folder = core.create_at_path("folder/").unwrap().id;
    let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();

    let doc = core.create_at_path("doc.md").unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    let doc2 = core.create_at_path("folder/doc.md").unwrap().id;
    let doc2 = core.db.local_metadata.get(&doc2).unwrap().unwrap();

    core.client
        .request(
            &account,
            UpsertRequest {
                updates: vec![FileDiff::new(&doc), FileDiff::new(&doc2), FileDiff::new(&folder)],
            },
        )
        .unwrap();

    let mut new = doc2.clone();
    new.timestamped_value.value.parent = root.id;
    new.timestamped_value.value.name = doc.timestamped_value.value.name;

    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc2, &new)] });

    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[test]
fn move_folder_into_itself() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let folder = core.create_at_path("folder/").unwrap().id;
    let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();

    core.client
        .request(&account, UpsertRequest { updates: vec![FileDiff::new(&folder)] })
        .unwrap();

    let mut new = folder.clone();
    new.timestamped_value.value.parent = *new.id();

    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&folder, &new)] });
    assert_matches!(
        result,
        Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(ValidationFailure::Cycle(
            _
        ))))
    );
}

#[test]
fn move_folder_into_descendants() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let folder = core.create_at_path("folder1/").unwrap().id;
    let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();

    let folder2 = core.create_at_path("folder1/folder2/").unwrap().id;
    let folder2 = core.db.local_metadata.get(&folder2).unwrap().unwrap();

    core.client
        .request(
            &account,
            UpsertRequest { updates: vec![FileDiff::new(&folder), FileDiff::new(&folder2)] },
        )
        .unwrap();

    let mut folder_new = folder.clone();
    folder_new.timestamped_value.value.parent = *folder2.id();
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&folder, &folder_new)] });
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

#[test]
fn move_document_into_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // create documents
    let doc = core.create_at_path("doc1.md").unwrap().id;
    let doc = core.db.local_metadata.get(&doc).unwrap().unwrap();

    let doc2 = core.create_at_path("doc2.md").unwrap().id;
    let doc2 = core.db.local_metadata.get(&doc2).unwrap().unwrap();

    core.client
        .request(
            &account,
            UpsertRequest { updates: vec![FileDiff::new(&doc), FileDiff::new(&doc2)] },
        )
        .unwrap();

    // move folder into itself
    let mut new = doc.clone();
    new.timestamped_value.value.parent = *doc2.id();
    let result = core
        .client
        .request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc, &new)] });
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}
