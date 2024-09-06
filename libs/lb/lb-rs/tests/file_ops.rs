use lb_rs::logic::filename::MAX_FILENAME_LENGTH;
use lb_rs::model::errors::CoreError;
use test_utils::{assert_matches, test_core_with_account};
use uuid::Uuid;

#[test]
fn rename() {
    let core = test_core_with_account();
    let id = core.create_at_path("doc.md").unwrap().id;
    assert_eq!(core.get_by_path("doc.md").unwrap().name, "doc.md");
    core.rename_file(id, "docs2.md").unwrap();
    assert_eq!(core.get_by_path("docs2.md").unwrap().name, "docs2.md");
}

#[test]
fn rename_not_found() {
    let core = test_core_with_account();
    let result = core.rename_file(Uuid::new_v4(), "test");
    assert_matches!(result.unwrap_err().kind, CoreError::FileNonexistent);
}

#[test]
fn rename_not_root() {
    let core = test_core_with_account();
    let result = core.rename_file(core.get_root().unwrap().id, "test");
    assert_matches!(result.unwrap_err().kind, CoreError::RootModificationInvalid);
}

#[test]
fn apply_rename_invalid_name() {
    let core = test_core_with_account();
    let id = core.create_at_path("doc.md").unwrap().id;
    assert_matches!(
        core.rename_file(id, "docs/2.md").unwrap_err().kind,
        CoreError::FileNameContainsSlash
    );
}

#[test]
fn name_taken() {
    let core = test_core_with_account();
    core.create_at_path("doc1.md").unwrap();
    let id = core.create_at_path("doc2.md").unwrap().id;
    assert_matches!(core.rename_file(id, "doc1.md").unwrap_err().kind, CoreError::PathTaken);
}

#[test]
fn name_empty() {
    let core = test_core_with_account();
    core.create_at_path("doc1.md").unwrap();
    let id = core.create_at_path("doc2.md").unwrap().id;
    assert_matches!(core.rename_file(id, "").unwrap_err().kind, CoreError::FileNameEmpty);
}

#[test]
fn name_invalid() {
    let core = test_core_with_account();
    let result = core.create_at_path(&"x".repeat(MAX_FILENAME_LENGTH + 1));
    assert!(result.is_err());
}

#[test]
fn mv() {
    let core = test_core_with_account();
    let id = core.create_at_path("folder/doc1.md").unwrap().id;
    core.move_file(id, core.get_root().unwrap().id).unwrap();
    core.get_by_path("doc1.md").unwrap();
}

#[test]
fn mv_not_found_parent() {
    let core = test_core_with_account();
    let id = core.create_at_path("folder/doc1.md").unwrap().id;
    assert_matches!(
        core.move_file(id, Uuid::new_v4()).unwrap_err().kind,
        CoreError::FileParentNonexistent
    );
}

#[test]
fn mv_not_found_target() {
    let core = test_core_with_account();
    assert_matches!(
        core.move_file(Uuid::new_v4(), core.get_root().unwrap().id)
            .unwrap_err()
            .kind,
        CoreError::FileNonexistent
    );
}

#[test]
fn move_parent_document() {
    let core = test_core_with_account();
    let id = core.create_at_path("folder/doc1.md").unwrap().id;
    let target = core.create_at_path("doc2.md").unwrap().id;
    assert_matches!(core.move_file(id, target).unwrap_err().kind, CoreError::FileNotFolder);
}

#[test]
fn move_root() {
    let core = test_core_with_account();
    let id = core.create_at_path("folder/").unwrap().id;
    assert_matches!(
        core.move_file(core.get_root().unwrap().id, id)
            .unwrap_err()
            .kind,
        CoreError::RootModificationInvalid
    );
}

#[test]
fn move_path_conflict() {
    let core = test_core_with_account();
    let dest = core.create_at_path("folder/test.md").unwrap().parent;
    let src = core.create_at_path("test.md").unwrap().id;
    assert_matches!(core.move_file(src, dest).unwrap_err().kind, CoreError::PathTaken);
}

#[test]
fn folder_into_self() {
    let core = test_core_with_account();
    let src = core.create_at_path("folder1/").unwrap().id;
    let dest = core.create_at_path("folder1/folder2/folder3/").unwrap().id;
    assert_matches!(core.move_file(src, dest).unwrap_err().kind, CoreError::FolderMovedIntoSelf);
}

#[test]
fn delete() {
    let core = test_core_with_account();
    assert_eq!(core.list_metadatas().unwrap().len(), 1);
    let id = core.create_at_path("test").unwrap().id;
    assert_eq!(core.list_metadatas().unwrap().len(), 2);
    core.delete_file(id).unwrap();
    assert_eq!(core.list_metadatas().unwrap().len(), 1);
}

#[test]
fn delete_root() {
    let core = test_core_with_account();
    assert_matches!(
        core.delete_file(core.get_root().unwrap().id)
            .unwrap_err()
            .kind,
        CoreError::RootModificationInvalid
    );
}
