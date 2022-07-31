use lockbook_core::CoreError;
use lockbook_shared::file_metadata::FileType;
use std::collections::HashMap;
use test_utils::test_core_with_account;

#[test]
fn apply_rename() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let document_id = document.id;
    files::apply_rename(&[root, folder, document].to_map(), document_id, "document2").unwrap();
}

#[test]
fn apply_rename_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let result = files::apply_rename(&[root, folder].to_map(), document.id, "document2");
    assert_eq!(result, Err(CoreError::FileNonexistent));
}

#[test]
fn apply_rename_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let root_id = root.id;
    let result = files::apply_rename(&[root, folder, document].to_map(), root_id, "root2");
    assert_eq!(result, Err(CoreError::RootModificationInvalid));
}

#[test]
fn apply_rename_invalid_name() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let document_id = document.id;
    let result =
        files::apply_rename(&[root, folder, document].to_map(), document_id, "invalid/name");
    assert_eq!(result, Err(CoreError::FileNameContainsSlash));
}

#[test]
fn apply_rename_path_conflict() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document1 = files::create(FileType::Document, root.id, "document1", &account.public_key());
    let document2 = files::create(FileType::Document, root.id, "document2", &account.public_key());

    let document1_id = document1.id;
    let result = files::apply_rename(
        &[root, folder, document1, document2].to_map(),
        document1_id,
        "document2",
    );
    assert_eq!(result, Err(CoreError::PathTaken));
}

#[test]
fn apply_move() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let folder_id = folder.id;
    let document_id = document.id;
    files::apply_move(&[root, folder, document].to_map(), document_id, folder_id).unwrap();
}

#[test]
fn apply_move_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let folder_id = folder.id;
    let document_id = document.id;
    let result = files::apply_move(&[root, folder].to_map(), document_id, folder_id);
    assert_eq!(result, Err(CoreError::FileNonexistent));
}

#[test]
fn apply_move_parent_not_found() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let folder_id = folder.id;
    let document_id = document.id;
    let result = files::apply_move(&[root, document].to_map(), document_id, folder_id);
    assert_eq!(result, Err(CoreError::FileParentNonexistent));
}

#[test]
fn apply_move_parent_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let document1 = files::create(FileType::Document, root.id, "document1", &account.public_key());
    let document2 = files::create(FileType::Document, root.id, "document2", &account.public_key());

    let document1_id = document1.id;
    let document2_id = document2.id;
    let result =
        files::apply_move(&[root, document1, document2].to_map(), document2_id, document1_id);
    assert_eq!(result, Err(CoreError::FileNotFolder));
}

#[test]
fn apply_move_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let folder_id = folder.id;
    let root_id = root.id;
    let result = files::apply_move(&[root, folder, document].to_map(), root_id, folder_id);
    assert_eq!(result, Err(CoreError::RootModificationInvalid));
}

#[test]
fn apply_move_path_conflict() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document1 = files::create(FileType::Document, root.id, "document", &account.public_key());
    let document2 = files::create(FileType::Document, folder.id, "document", &account.public_key());

    let folder_id = folder.id;
    let document1_id = document1.id;
    let result =
        files::apply_move(&[root, folder, document1, document2].to_map(), document1_id, folder_id);
    assert_eq!(result, Err(CoreError::PathTaken));
}

#[test]
fn apply_move_2cycle() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder1 = files::create(FileType::Folder, root.id, "folder1", &account.public_key());
    let folder2 = files::create(FileType::Folder, folder1.id, "folder2", &account.public_key());

    let folder1_id = folder1.id;
    let folder2_id = folder2.id;
    let result = files::apply_move(&[root, folder1, folder2].to_map(), folder1_id, folder2_id);
    assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
}

#[test]
fn hash_map_apply_move_2cycle() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let mut folder1 = files::create(FileType::Folder, root.id, "folder1", &account.public_key());
    let folder2 = files::create(FileType::Folder, folder1.id, "folder2", &account.public_key());

    folder1.parent = folder2.id;
    let all_files =
        HashMap::from([(root.id, root), (folder1.id, folder1.clone()), (folder2.id, folder2)]);
    // let all_files = &[root.clone(), folder1.clone(), folder2.clone()];
    let result = all_files
        .get_invalid_cycles(&all_files)
        .unwrap()
        .contains(&folder1.id);
    assert!(result);
}

#[test]
fn apply_move_1cycle() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder1", &account.public_key());

    let folder1_id = folder.id;
    let result = files::apply_move(&[root, folder].to_map(), folder1_id, folder1_id);
    assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
}

#[test]
fn apply_delete() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let document_id = document.id;
    files::apply_delete(&[root, folder, document].to_map(), document_id).unwrap();
}

#[test]
fn apply_delete_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let document = files::create(FileType::Document, root.id, "document", &account.public_key());

    let root_id = root.id;
    let result = files::apply_delete(&[root, folder, document].to_map(), root_id);
    assert_eq!(result, Err(CoreError::RootModificationInvalid));
}

#[test]
fn get_nonconflicting_filename() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    assert_eq!(
        files::suggest_non_conflicting_filename(
            folder.id,
            &[root, folder].to_map(),
            &HashMap::new()
        )
        .unwrap(),
        "folder-1"
    );
}

#[test]
fn get_nonconflicting_filename2() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let folder2 = files::create(FileType::Folder, root.id, "folder-1", &account.public_key());
    assert_eq!(
        files::suggest_non_conflicting_filename(
            folder1.id,
            &[root, folder1, folder2].to_map(),
            &HashMap::new()
        )
        .unwrap(),
        "folder-2"
    );
}

#[test]
fn get_path_conflicts_no_conflicts() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let folder2 = files::create(FileType::Folder, root.id, "folder2", &account.public_key());

    let path_conflicts = &[root, folder1]
        .to_map()
        .get_path_conflicts(&[folder2].to_map())
        .unwrap();

    assert_eq!(path_conflicts.len(), 0);
}

#[test]
fn get_path_conflicts_one_conflict() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = files::create_root(&account);
    let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
    let folder2 = files::create(FileType::Folder, root.id, "folder", &account.public_key());

    let path_conflicts = &[root, folder1.clone()]
        .to_map()
        .get_path_conflicts(&[folder2.clone()].to_map())
        .unwrap();

    assert_eq!(path_conflicts.len(), 1);
    assert_eq!(path_conflicts[0], PathConflict { existing: folder1.id, staged: folder2.id });
}
