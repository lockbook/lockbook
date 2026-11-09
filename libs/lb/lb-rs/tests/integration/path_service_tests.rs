use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::path_ops::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use test_utils::*;

#[tokio::test]
async fn create_at_path_document() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    assert_eq!(doc.file_type, FileType::Document);
}

#[tokio::test]
async fn create_at_path_folder() {
    let core = test_core_with_account().await;
    let folder = core.create_at_path("/folder/").await.unwrap();
    assert_eq!(folder.file_type, FileType::Folder);
}

#[tokio::test]
async fn create_at_path_in_folder() {
    let core = test_core_with_account().await;

    let folder = core.create_at_path("/folder/").await.unwrap();
    let document = core.create_at_path("/folder/document").await.unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[tokio::test]
async fn create_at_path_missing_folder() {
    let core = test_core_with_account().await;

    let document = core.create_at_path("/folder/document").await.unwrap();
    let folder = core.get_by_path("/folder").await.unwrap();

    assert_eq!(folder.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[tokio::test]
async fn create_at_path_missing_folders() {
    let core = test_core_with_account().await;

    let document = core
        .create_at_path("/folder/folder/document")
        .await
        .unwrap();
    let folder1 = core.get_by_path("/folder").await.unwrap();
    let folder2 = core.get_by_path("/folder/folder").await.unwrap();

    assert_eq!(folder1.file_type, FileType::Folder);
    assert_eq!(folder2.file_type, FileType::Folder);
    assert_eq!(document.file_type, FileType::Document);
}

#[tokio::test]
async fn create_at_path_path_contains_empty_file_name() {
    let core = test_core_with_account().await;
    let result = core.create_at_path("//document").await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::PathContainsEmptyFileName);
}

#[tokio::test]
async fn create_at_path_path_taken() {
    let core = test_core_with_account().await;
    core.create_at_path("/folder/document").await.unwrap();
    let result = core.create_at_path("/folder/document").await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::PathConflict(_))
    );
}

#[tokio::test]
async fn create_at_path_not_folder() {
    let core = test_core_with_account().await;
    core.create_at_path("/not-folder").await.unwrap();
    let result = core.create_at_path("/not-folder/document").await;
    assert_matches!(
        result.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(_))
    );
}

#[tokio::test]
async fn get_by_path_document() {
    let core = test_core_with_account().await;
    let created_document = core.create_at_path("/document").await.unwrap();
    let document = core.get_by_path("/document").await.unwrap();
    assert_eq!(created_document, document);
}

#[tokio::test]
async fn get_by_path_folder() {
    let core = test_core_with_account().await;
    let created_folder = core.create_at_path("/folder/").await.unwrap();
    let folder = core.get_by_path("/folder").await.unwrap();
    assert_eq!(created_folder, folder);
}

#[tokio::test]
async fn get_by_path_document_in_folder() {
    let core = test_core_with_account().await;
    let created_document = core.create_at_path("/folder/document").await.unwrap();
    let document = core.get_by_path("/folder/document").await.unwrap();
    assert_eq!(created_document, document);
}

#[tokio::test]
async fn get_path_by_id_document() {
    let core = test_core_with_account().await;
    let document = core.create_at_path("/document").await.unwrap();
    let document_path = core.get_path_by_id(document.id).await.unwrap();
    assert_eq!(&document_path, "/document");
}

#[tokio::test]
async fn get_path_by_id_folder() {
    let core = test_core_with_account().await;
    let folder = core.create_at_path("/folder/").await.unwrap();
    let folder_path = core.get_path_by_id(folder.id).await.unwrap();
    assert_eq!(&folder_path, "/folder/");
}

#[tokio::test]
async fn get_path_by_id_document_in_folder() {
    let core = test_core_with_account().await;
    let document = core.create_at_path("/folder/document").await.unwrap();
    let document_path = core.get_path_by_id(document.id).await.unwrap();
    assert_eq!(&document_path, "/folder/document");
}

#[tokio::test]
async fn get_all_paths() {
    let core = test_core_with_account().await;

    core.create_at_path("/folder/folder/document")
        .await
        .unwrap();
    core.create_at_path("/folder/folder/folder/").await.unwrap();

    let all_paths = core.list_paths(None).await.unwrap();
    assert!(all_paths.iter().any(|p| p == "/"));
    assert!(all_paths.iter().any(|p| p == "/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert_eq!(all_paths.len(), 5);
}

#[tokio::test]
async fn get_all_paths_documents_only() {
    let core = test_core_with_account().await;

    core.create_at_path("/folder/folder/document")
        .await
        .unwrap();
    core.create_at_path("/folder/folder/folder/").await.unwrap();

    let all_paths = core.list_paths(Some(DocumentsOnly)).await.unwrap();
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert_eq!(all_paths.len(), 1);
}

#[tokio::test]
async fn get_all_paths_folders_only() {
    let core = test_core_with_account().await;

    core.create_at_path("/folder/folder/document")
        .await
        .unwrap();
    core.create_at_path("/folder/folder/folder/").await.unwrap();

    let all_paths = core.list_paths(Some(FoldersOnly)).await.unwrap();
    assert!(all_paths.iter().any(|p| p == "/"));
    assert!(all_paths.iter().any(|p| p == "/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert_eq!(all_paths.len(), 4);
}

#[tokio::test]
async fn get_all_paths_leaf_nodes_only() {
    let core = test_core_with_account().await;

    core.create_at_path("/folder/folder/document")
        .await
        .unwrap();
    core.create_at_path("/folder/folder/folder/").await.unwrap();

    let all_paths = core.list_paths(Some(LeafNodesOnly)).await.unwrap();
    assert!(all_paths.iter().any(|p| p == "/folder/folder/folder/"));
    assert!(all_paths.iter().any(|p| p == "/folder/folder/document"));
    assert_eq!(all_paths.len(), 2);
}
