use lockbook_core::model::errors::CreateFileAtPathError::*;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::pure_functions::files;
use lockbook_core::service::path_service::Filter;
use lockbook_core::service::path_service::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::service::{file_service, path_service};
use lockbook_core::Error::UiError;
use lockbook_core::{CoreError, Error};
use lockbook_models::file_metadata::FileType;
use test_utils::*;

#[test]
fn create_at_path_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let doc = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();

    assert_eq!(doc.file_type, FileType::Document);
}

#[test]
fn create_at_path_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
}

#[test]
fn create_at_path_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let folder = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folders() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    let folder1 = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();
    let folder2 = core
        .get_by_path(&format!("{}/folder/folder", &account.username))
        .unwrap();

    assert_eq!(folder1.file_type, FileType::Folder);
    assert_eq!(folder2.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_path_contains_empty_file_name() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let result = core.create_at_path(&format!("{}//document", &account.username));

    assert_matches!(result, Err(UiError(PathContainsEmptyFile)));
}

#[test]
fn create_at_path_path_starts_with_non_root() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let result = core.create_at_path(&format!("{}/folder/document", "not-account-username"));

    assert_matches!(result, Err(UiError(PathDoesntStartWithRoot)));
}

#[test]
fn create_at_path_path_taken() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let result = core.create_at_path(&format!("{}/folder/document", &account.username));

    assert_matches!(result, Err(UiError(FileAlreadyExists)));
}

#[test]
fn create_at_path_not_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/not-folder", &account.username))
        .unwrap();
    let result = core.create_at_path(&format!("{}/not-folder/document", &account.username));

    assert_matches!(result, Err(UiError(DocumentTreatedAsFolder)));
}

#[test]
fn get_by_path_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let created_document = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();
    let document = core
        .get_by_path(&format!("{}/document", &account.username))
        .unwrap();

    assert_eq!(created_document, document);
}

#[test]
fn get_by_path_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let created_folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let folder = core
        .get_by_path(&format!("{}/folder", &account.username))
        .unwrap();

    assert_eq!(created_folder, folder);
}

#[test]
fn get_by_path_document_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let created_document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let document = core
        .get_by_path(&format!("{}/folder/document", &account.username))
        .unwrap();

    assert_eq!(created_document, document);
}

#[test]
fn get_path_by_id_document() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let document = core
        .create_at_path(&format!("{}/document", &account.username))
        .unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();

    assert_eq!(&document_path, &format!("{}/document", &account.username));
}

#[test]
fn get_path_by_id_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let folder = core
        .create_at_path(&format!("{}/folder/", &account.username))
        .unwrap();
    let folder_path = core.get_path_by_id(folder.id).unwrap();

    assert_eq!(&folder_path, &format!("{}/folder/", &account.username));
}

#[test]
fn get_path_by_id_document_in_folder() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    let document = core
        .create_at_path(&format!("{}/folder/document", &account.username))
        .unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();

    assert_eq!(&document_path, &format!("{}/folder/document", &account.username));
}

#[test]
fn get_all_paths() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(None).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
    assert_eq!(all_paths.len(), 5);
}

#[test]
fn get_all_paths_documents_only() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(DocumentsOnly)).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
    assert_eq!(all_paths.len(), 1);
}

#[test]
fn get_all_paths_folders_only() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(FoldersOnly)).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
    assert_eq!(all_paths.len(), 4);
}

#[test]
fn get_all_paths_leaf_nodes_only() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    core.create_at_path(&format!("{}/folder/folder/document", &account.username))
        .unwrap();
    core.create_at_path(&format!("{}/folder/folder/folder/", &account.username))
        .unwrap();

    let all_paths = core.list_paths(Some(LeafNodesOnly)).unwrap();
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/folder/", &account.username)));
    assert!(all_paths
        .iter()
        .any(|p| p == &format!("{}/folder/folder/document", &account.username)));
    assert_eq!(all_paths.len(), 2);
}
