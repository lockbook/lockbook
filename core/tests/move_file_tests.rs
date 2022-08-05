use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use test_utils::*;
use uuid::Uuid;

use lockbook_shared::api::*;
use lockbook_shared::file_metadata::FileDiff;

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
    api_service::request(&account, UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] })
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

    let result = api_service::request(
        &account,
        UpsertRequest { updates: vec![FileDiff::edit(&doc1, &doc2)] },
    );
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::Validation(_))));
}

// #[test]
// fn move_document_deleted() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//
//     let doc = core.create_at_path("doc.md").unwrap().id;
//     let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
//
//     let folder = core.create_at_path("folder/").unwrap().id;
//     let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::new(&doc), FileDiff::new(&folder)] },
//     )
//     .unwrap();
//
//     // move & delete document
//     doc.is_deleted = true;
//     doc.parent = folder.id;
//     api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &doc.name, &doc)] },
//     )
//     .unwrap();
// }
//
// #[test]
// fn move_document_conflict() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//
//     let doc = core.create_at_path("doc.md").unwrap().id;
//     let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
//
//     let folder = core.create_at_path("folder/").unwrap().id;
//     let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::new(&doc), FileDiff::new(&folder)] },
//     )
//     .unwrap();
//
//     // move document
//     doc.parent = folder.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest {
//             // use incorrect previous parent
//             updates: vec![FileDiff::edit(folder.id, &doc.name, &doc)],
//         },
//     );
//     assert_matches!(result, UPDATES_REQ);
// }
//
// #[test]
// fn move_document_path_taken() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//
//     let folder = core.create_at_path("folder/").unwrap().id;
//     let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     let doc = core.create_at_path("doc.md").unwrap().id;
//     let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
//
//     let doc2 = core.create_at_path("folder/doc.md").unwrap().id;
//     let mut doc2 = core.db.local_metadata.get(&doc2).unwrap().unwrap();
//     doc2.name = doc.name.clone();
//
//     api_service::request(
//         &account,
//         UpsertRequest {
//             updates: vec![FileDiff::new(&doc), FileDiff::new(&doc2), FileDiff::new(&folder)],
//         },
//     )
//     .unwrap();
//
//     // move document
//     doc.parent = folder.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &doc.name, &doc)] },
//     );
//     assert_matches!(result, UPDATES_REQ);
// }
//
// #[test]
// fn move_folder_cannot_move_root() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//     let mut root = core.db.base_metadata.get(&root.id).unwrap().unwrap();
//
//     let folder = core.create_at_path("folder/").unwrap().id;
//     let folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     api_service::request(&account, UpsertRequest { updates: vec![FileDiff::new(&folder)] })
//         .unwrap();
//
//     // move root
//     root.parent = folder.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &root.name, &root)] },
//     );
//     assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::RootImmutable)));
// }
//
// #[test]
// fn move_folder_into_itself() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//
//     let folder = core.create_at_path("folder/").unwrap().id;
//     let mut folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     api_service::request(&account, UpsertRequest { updates: vec![FileDiff::new(&folder)] })
//         .unwrap();
//
//     folder.parent = folder.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &folder.name, &folder)] },
//     );
//     assert_matches!(result, UPDATES_REQ);
// }
//
// #[test]
// fn move_folder_into_descendants() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//
//     let folder = core.create_at_path("folder1/").unwrap().id;
//     let mut folder = core.db.local_metadata.get(&folder).unwrap().unwrap();
//
//     let folder2 = core.create_at_path("folder1/folder2/").unwrap().id;
//     let folder2 = core.db.local_metadata.get(&folder2).unwrap().unwrap();
//
//     api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::new(&folder), FileDiff::new(&folder2)] },
//     )
//     .unwrap();
//
//     // move folder into itself
//     folder.parent = folder2.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &folder.name, &folder)] },
//     );
//     assert_matches!(result, UPDATES_REQ);
// }
//
// #[test]
// fn move_document_into_document() {
//     let core = test_core_with_account();
//     let account = core.get_account().unwrap();
//     let root = core.get_root().unwrap();
//
//     // create documents
//     let doc = core.create_at_path("doc1.md").unwrap().id;
//     let mut doc = core.db.local_metadata.get(&doc).unwrap().unwrap();
//
//     let doc2 = core.create_at_path("doc2.md").unwrap().id;
//     let doc2 = core.db.local_metadata.get(&doc2).unwrap().unwrap();
//
//     api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::new(&doc), FileDiff::new(&doc2)] },
//     )
//     .unwrap();
//
//     // move folder into itself
//     doc.parent = doc2.id;
//     let result = api_service::request(
//         &account,
//         UpsertRequest { updates: vec![FileDiff::edit(root.id, &doc.name, &doc)] },
//     );
//     assert_matches!(result, UPDATES_REQ);
// }
