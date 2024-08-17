use lb_rs::CoreError;
use lb_rs::shared::file_metadata::FileType;
use lb_rs::shared::path_ops::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use test_utils::*;

#[test]
fn create_at_path_document() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    assert_eq!(doc.file_type, FileType::Document);
}

#[test]
fn create_at_path_folder() {
    let core = test_core_with_account();
    let folder = core.create_at_path("/folder/").unwrap();
    assert_eq!(folder.file_type, FileType::Folder);
}

#[test]
fn create_at_path_in_folder() {
    let core = test_core_with_account();

    let folder = core.create_at_path("/folder/").unwrap();
    let document = core.create_at_path("/folder/document").unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folder() {
    let core = test_core_with_account();

    let document = core.create_at_path("/folder/document").unwrap();
    let folder = core.get_by_path("/folder").unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_missing_folders() {
    let core = test_core_with_account();

    let document = core.create_at_path("/folder/folder/document").unwrap();
    let folder1 = core.get_by_path("/folder").unwrap();
    let folder2 = core.get_by_path("/folder/folder").unwrap();

    assert_eq!(folder1.file_type, FileType::Folder);
    assert_eq!(folder2.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[test]
fn create_at_path_path_contains_empty_file_name() {
    let core = test_core_with_account();
    let result = core.create_at_path("//document");
    assert_matches!(result.unwrap_err().kind, CoreError::PathContainsEmptyFileName);
}

#[test]
fn create_at_path_path_taken() {
    let core = test_core_with_account();
    core.create_at_path("/folder/document").unwrap();
    let result = core.create_at_path("/folder/document");
    assert_matches!(result.unwrap_err().kind, CoreError::PathTaken);
}

#[test]
fn create_at_path_not_folder() {
    let core = test_core_with_account();
    core.create_at_path("/not-folder").unwrap();
    let result = core.create_at_path("/not-folder/document");
    assert_matches!(result.unwrap_err().kind, CoreError::FileNotFolder);
}

#[test]
fn get_by_path_document() {
    let core = test_core_with_account();
    let created_document = core.create_at_path("/document").unwrap();
    let document = core.get_by_path("/document").unwrap();
    assert_eq!(created_document, document);
}

#[test]
fn get_by_path_folder() {
    let core = test_core_with_account();
    let created_folder = core.create_at_path("/folder/").unwrap();
    let folder = core.get_by_path("/folder").unwrap();
    assert_eq!(created_folder, folder);
}

#[test]
fn get_by_path_document_in_folder() {
    let core = test_core_with_account();
    let created_document = core.create_at_path("/folder/document").unwrap();
    let document = core.get_by_path("/folder/document").unwrap();
    assert_eq!(created_document, document);
}

#[test]
fn get_path_by_id_document() {
    let core = test_core_with_account();
    let document = core.create_at_path("/document").unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();
    assert_eq!(&document_path, "/document");
}

#[test]
fn get_path_by_id_folder() {
    let core = test_core_with_account();
    let folder = core.create_at_path("/folder/").unwrap();
    let folder_path = core.get_path_by_id(folder.id).unwrap();
    assert_eq!(&folder_path, "/folder/");
}

#[test]
fn get_path_by_id_document_in_folder() {
    let core = test_core_with_account();
    let document = core.create_at_path("/folder/document").unwrap();
    let document_path = core.get_path_by_id(document.id).unwrap();
    assert_eq!(&document_path, "/folder/document");
}

#[test]
fn get_all_paths() {
    let core = test_core_with_account();

    core.create_at_path("/folder/folder/document").unwrap();
    core.create_at_path("/folder/folder/folder/").unwrap();

    let all_paths = core.list_paths(None).unwrap();
    assert!(all_paths.iter().any(|p| p == "/"));
    assert!(all_paths.iter().any(|p| p == "/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert_eq!(all_paths.len(), 5);
}

#[test]
fn get_all_paths_documents_only() {
    let core = test_core_with_account();

    core.create_at_path("/folder/folder/document").unwrap();
    core.create_at_path("/folder/folder/folder/").unwrap();

    let all_paths = core.list_paths(Some(DocumentsOnly)).unwrap();
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert_eq!(all_paths.len(), 1);
}

#[test]
fn get_all_paths_folders_only() {
    let core = test_core_with_account();

    core.create_at_path("/folder/folder/document").unwrap();
    core.create_at_path("/folder/folder/folder/").unwrap();

    let all_paths = core.list_paths(Some(FoldersOnly)).unwrap();
    assert!(all_paths.iter().any(|p| p == "/"));
    assert!(all_paths.iter().any(|p| p == "/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert_eq!(all_paths.len(), 4);
}

#[test]
fn get_all_paths_leaf_nodes_only() {
    let core = test_core_with_account();

    core.create_at_path("/folder/folder/document").unwrap();
    core.create_at_path("/folder/folder/folder/").unwrap();

    let all_paths = core.list_paths(Some(LeafNodesOnly)).unwrap();
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert_eq!(all_paths.len(), 2);
}
