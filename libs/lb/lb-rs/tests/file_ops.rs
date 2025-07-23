use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::filename::MAX_FILENAME_LENGTH;
use test_utils::{assert_matches, test_core_with_account};
use uuid::Uuid;

#[tokio::test]
async fn rename() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("doc.md").await.unwrap().id;
    assert_eq!(core.get_by_path("doc.md").await.unwrap().name, "doc.md");
    core.rename_file(&id, "docs2.md").await.unwrap();
    assert_eq!(core.get_by_path("docs2.md").await.unwrap().name, "docs2.md");
}

#[tokio::test]
async fn rename_not_found() {
    let core = test_core_with_account().await;
    let result = core.rename_file(&Uuid::new_v4(), "test").await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::FileNonexistent);
}

#[tokio::test]
async fn rename_not_root() {
    let core = test_core_with_account().await;
    let result = core
        .rename_file(&core.root().await.unwrap().id, "test")
        .await;
    assert_matches!(result.unwrap_err().kind, LbErrKind::RootModificationInvalid);
}

#[tokio::test]
async fn apply_rename_invalid_name() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("doc.md").await.unwrap().id;
    assert_matches!(
        core.rename_file(&id, "docs/2.md").await.unwrap_err().kind,
        LbErrKind::FileNameContainsSlash
    );
}

#[tokio::test]
async fn name_taken() {
    let core = test_core_with_account().await;
    core.create_at_path("doc1.md").await.unwrap();
    let id = core.create_at_path("doc2.md").await.unwrap().id;
    assert_matches!(
        core.rename_file(&id, "doc1.md").await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::PathConflict(_))
    );
}

#[tokio::test]
async fn name_empty() {
    let core = test_core_with_account().await;
    core.create_at_path("doc1.md").await.unwrap();
    let id = core.create_at_path("doc2.md").await.unwrap().id;
    assert_matches!(core.rename_file(&id, "").await.unwrap_err().kind, LbErrKind::FileNameEmpty);
}

#[tokio::test]
async fn name_invalid() {
    let core = test_core_with_account().await;
    let result = core
        .create_at_path(&"x".repeat(MAX_FILENAME_LENGTH + 1))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn mv() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("folder/doc1.md").await.unwrap().id;
    core.move_file(&id, &core.root().await.unwrap().id)
        .await
        .unwrap();
    core.get_by_path("doc1.md").await.unwrap();
}

#[tokio::test]
async fn mv_not_found_parent() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("folder/doc1.md").await.unwrap().id;
    assert_matches!(
        core.move_file(&id, &Uuid::new_v4()).await.unwrap_err().kind,
        LbErrKind::FileParentNonexistent
    );
}

#[tokio::test]
async fn mv_not_found_target() {
    let core = test_core_with_account().await;
    assert_matches!(
        core.move_file(&Uuid::new_v4(), &core.root().await.unwrap().id)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::FileNonexistent
    );
}

#[tokio::test]
async fn move_parent_document() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("folder/doc1.md").await.unwrap().id;
    let target = core.create_at_path("doc2.md").await.unwrap().id;
    assert_matches!(
        core.move_file(&id, &target).await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(_))
    );
}

#[tokio::test]
async fn move_root() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("folder/").await.unwrap().id;
    assert_matches!(
        core.move_file(&core.root().await.unwrap().id, &id)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::RootModificationInvalid
    );
}

#[tokio::test]
async fn move_path_conflict() {
    let core = test_core_with_account().await;
    let dest = core.create_at_path("folder/test.md").await.unwrap().parent;
    let src = core.create_at_path("test.md").await.unwrap().id;
    assert_matches!(
        core.move_file(&src, &dest).await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::PathConflict(_))
    );
}

#[tokio::test]
async fn folder_into_self() {
    let core = test_core_with_account().await;
    let src = core.create_at_path("folder1/").await.unwrap().id;
    let dest = core
        .create_at_path("folder1/folder2/folder3/")
        .await
        .unwrap()
        .id;
    assert_matches!(
        core.move_file(&src, &dest).await.unwrap_err().kind,
        LbErrKind::Validation(ValidationFailure::Cycle(_))
    );
}

#[tokio::test]
async fn delete() {
    let core = test_core_with_account().await;
    assert_eq!(core.list_metadatas().await.unwrap().len(), 1);
    let id = core.create_at_path("test").await.unwrap().id;
    assert_eq!(core.list_metadatas().await.unwrap().len(), 2);
    core.delete(&id).await.unwrap();
    assert_eq!(core.list_metadatas().await.unwrap().len(), 1);
}

#[tokio::test]
async fn delete_root() {
    let core = test_core_with_account().await;
    assert_matches!(
        core.delete(&core.root().await.unwrap().id)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::RootModificationInvalid
    );
}
