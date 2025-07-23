use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::path_ops::Filter;
use test_utils::*;

#[tokio::test]
async fn test_create_delete_list() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("test.md").await.unwrap().id;
    assert_eq!(
        core.list_paths(Some(Filter::LeafNodesOnly))
            .await
            .unwrap()
            .len(),
        1
    );
    core.delete(&id).await.unwrap();
    assert_eq!(
        core.list_paths(Some(Filter::LeafNodesOnly))
            .await
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn test_create_delete_read() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("test.md").await.unwrap().id;
    core.delete(&id).await.unwrap();
    assert_matches!(
        core.read_document(id, false).await.unwrap_err().kind,
        LbErrKind::FileNonexistent
    );
}

#[tokio::test]
async fn test_create_delete_write() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("test.md").await.unwrap().id;
    core.delete(&id).await.unwrap();
    assert_matches!(
        core.write_document(id, "document content".as_bytes())
            .await
            .unwrap_err()
            .kind,
        LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(_))
    );
}

#[tokio::test]
async fn test_create_parent_delete_create_in_parent() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("folder/").await.unwrap().id;
    core.delete(&id).await.unwrap();

    assert_matches!(
        core.create_file("document", &id, FileType::Document)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::FileParentNonexistent
    );
}

#[tokio::test]
async fn try_to_delete_root() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    assert_matches!(
        core.delete(&root.id).await.unwrap_err().kind,
        LbErrKind::RootModificationInvalid
    );
}

#[tokio::test]
async fn test_create_parent_delete_parent_read_doc() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    core.write_document(doc.id, "content".as_bytes())
        .await
        .unwrap();
    assert_eq!(core.read_document(doc.id, false).await.unwrap(), "content".as_bytes());
    core.delete(&doc.parent).await.unwrap();
    assert_matches!(
        core.read_document(doc.id, false).await.unwrap_err().kind,
        LbErrKind::FileNonexistent
    );
}

#[tokio::test]
async fn test_create_parent_delete_parent_rename_doc() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    core.delete(&doc.parent).await.unwrap();
    assert_matches!(
        core.rename_file(&doc.id, "test2.md")
            .await
            .unwrap_err()
            .kind,
        LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(_))
    );
}

#[tokio::test]
async fn test_create_parent_delete_parent_rename_parent() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    core.delete(&doc.parent).await.unwrap();
    assert_matches!(
        core.rename_file(&doc.parent, "folder2")
            .await
            .unwrap_err()
            .kind,
        LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(_))
    );
}

#[tokio::test]
async fn test_folder_move_delete_source_doc() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    let folder2 = core.create_at_path("folder2/").await.unwrap();
    core.delete(&doc.parent).await.unwrap();
    assert_matches!(
        core.move_file(&doc.id, &folder2.id).await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(_))
    );
}

#[tokio::test]
async fn test_folder_move_delete_source_parent() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    let folder2 = core.create_at_path("folder2/").await.unwrap();
    core.delete(&doc.parent).await.unwrap();
    assert_matches!(
        core.move_file(&doc.parent, &folder2.id)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(_))
    );
}

#[tokio::test]
async fn test_folder_move_delete_destination_parent() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    let folder2 = core.create_at_path("folder2/").await.unwrap();
    core.delete(&folder2.id).await.unwrap();
    assert_matches!(
        core.move_file(&doc.id, &folder2.id).await.unwrap_err().kind,
        LbErrKind::FileParentNonexistent
    );
}

#[tokio::test]
async fn test_folder_move_delete_destination_doc() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("folder/test.md").await.unwrap();
    let folder2 = core.create_at_path("folder2/").await.unwrap();
    core.delete(&folder2.id).await.unwrap();
    assert_matches!(
        core.move_file(&doc.parent, &folder2.id)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::FileParentNonexistent
    );
}

#[tokio::test]
async fn test_delete_list_files() {
    let core = test_core_with_account().await;
    let f1 = core.create_at_path("f1/").await.unwrap();
    core.create_at_path("f1/f2/").await.unwrap();
    let d1 = core.create_at_path("f1/f2/d1.md").await.unwrap();
    core.delete(&f1.id).await.unwrap();

    let mut files = core.list_metadatas().await.unwrap();
    files.retain(|meta| meta.id == d1.id);

    assert!(files.is_empty());
}

#[tokio::test]
async fn test_write_delete_sync_doc() {
    let core = test_core_with_account().await;

    let doc = core.create_at_path("test.md").await.unwrap().id;
    core.write_document(doc, &[1, 2, 3]).await.unwrap();
    core.delete(&doc).await.unwrap();
    core.sync(None).await.unwrap();
}
