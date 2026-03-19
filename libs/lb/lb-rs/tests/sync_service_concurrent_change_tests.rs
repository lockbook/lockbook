use lb_rs::Lb;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::FileType;
use test_utils::*;

/// Tests that setup two synced devices, operate on both devices, then sync both twice (work
/// should be none, devices dbs should be equal, deleted files should be pruned).
async fn sync_and_assert(c1: &Lb, c2: &Lb) {
    c1.sync(None).await.unwrap();
    c2.sync(None).await.unwrap();
    c1.sync(None).await.unwrap();
    c2.sync(None).await.unwrap();

    c1.test_repo_integrity(true).await.unwrap();
    assert::cores_equal(c1, c2).await;
    assert::local_work_paths(c1, &[]).await;
    assert::server_work_paths(c1, &[]).await;
    assert::deleted_files_pruned(c1);
}

#[tokio::test]
async fn identical_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document", "/parent/").await.unwrap();
    move_by_path(&c2, "/document", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document"]).await;
    assert::all_document_contents(&c2, &[("/parent/document", b"")]).await;
}

#[tokio::test]
async fn different_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/parent2/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document", "/parent/").await.unwrap();
    move_by_path(&c2, "/document", "/parent2/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent2/", "/parent/document"]).await;
    assert::all_document_contents(&c2, &[("/parent/document", b"")]).await;
}

#[tokio::test]
async fn identical_rename() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/document", "document2").await.unwrap();
    rename_path(&c2, "/document", "document2").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document2"]).await;
    assert::all_document_contents(&c2, &[("/document2", b"")]).await;
}

#[tokio::test]
async fn different_rename() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/document", "document2").await.unwrap();
    rename_path(&c2, "/document", "document3").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document2"]).await;
    assert::all_document_contents(&c2, &[("/document2", b"")]).await;
}

#[tokio::test]
async fn move_then_rename() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document", "/parent/").await.unwrap();
    rename_path(&c2, "/document", "document2").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document2"]).await;
    assert::all_document_contents(&c2, &[("/parent/document2", b"")]).await;
}

#[tokio::test]
async fn rename_then_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/document", "document2").await.unwrap();
    move_by_path(&c2, "/document", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document2"]).await;
    assert::all_document_contents(&c2, &[("/parent/document2", b"")]).await;
}

#[tokio::test]
async fn identical_delete() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/document").await.unwrap();
    delete_path(&c2, "/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn identical_delete_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_parent_then_direct() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    delete_path(&c2, "/parent/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_direct_then_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/document").await.unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn identical_delete_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/").await.unwrap();
    delete_path(&c2, "/grandparent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_grandparent_then_direct() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/").await.unwrap();
    delete_path(&c2, "/grandparent/parent/document")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_direct_then_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/parent/document")
        .await
        .unwrap();
    delete_path(&c2, "/grandparent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_grandparent_then_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/").await.unwrap();
    delete_path(&c2, "/grandparent/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_parent_then_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/parent/").await.unwrap();
    delete_path(&c2, "/grandparent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn move_then_delete() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document", "/parent/").await.unwrap();
    delete_path(&c2, "/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_then_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/document").await.unwrap();
    move_by_path(&c2, "/document", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn move_then_delete_new_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document", "/parent/").await.unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_new_parent_then_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    move_by_path(&c2, "/document", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn move_then_delete_old_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/parent/document", "").await.unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document"]).await;
    assert::all_document_contents(&c2, &[("/document", b"")]).await;
}

#[tokio::test]
async fn delete_old_parent_then_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    move_by_path(&c2, "/parent/document", "").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn rename_then_delete() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/document", "document2").await.unwrap();
    delete_path(&c2, "/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_then_rename() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/document").await.unwrap();
    rename_path(&c2, "/document", "document2").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn create_then_move_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/parent2/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/parent/document").await.unwrap();
    move_by_path(&c2, "/parent/", "/parent2/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"])
        .await;
    assert::all_document_contents(&c2, &[("/parent2/parent/document", b"")]).await;
}

#[tokio::test]
async fn move_parent_then_create() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/parent2/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/parent/", "/parent2/").await.unwrap();
    c2.create_at_path("/parent/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"])
        .await;
    assert::all_document_contents(&c2, &[("/parent2/parent/document", b"")]).await;
}

#[tokio::test]
async fn create_then_rename_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/parent/document").await.unwrap();
    rename_path(&c2, "/parent/", "parent2").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/document"]).await;
    assert::all_document_contents(&c2, &[("/parent2/document", b"")]).await;
}

#[tokio::test]
async fn rename_parent_then_create() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/parent/", "parent2").await.unwrap();
    c2.create_at_path("/parent/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/document"]).await;
    assert::all_document_contents(&c2, &[("/parent2/document", b"")]).await;
}

#[tokio::test]
async fn create_then_delete_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/parent/document").await.unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_parent_then_create() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    c2.create_at_path("/parent/document").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn create_then_delete_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    delete_path(&c2, "/grandparent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_grandparent_then_create() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/").await.unwrap();
    c2.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn identical_content_edit_not_mergable() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.draw").await.unwrap();
    write_path(&c1, "/document.draw", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.draw", b"document content 2")
        .await
        .unwrap();
    write_path(&c2, "/document.draw", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document.draw"]).await;
    assert::all_document_contents(&c2, &[("/document.draw", b"document content 2")]).await;
}

#[tokio::test]
async fn identical_content_edit_mergable() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document content 2")
        .await
        .unwrap();
    write_path(&c2, "/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document.md"]).await;
    assert::all_document_contents(&c2, &[("/document.md", b"document content 2")]).await;
}

#[tokio::test]
async fn different_content_edit_not_mergable() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.draw").await.unwrap();
    write_path(&c1, "/document.draw", b"document\n\ncontent\n")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.draw", b"document 2\n\ncontent\n")
        .await
        .unwrap();
    write_path(&c2, "/document.draw", b"document\n\ncontent 2\n")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document.draw", "/document-1.draw"]).await;
    assert::all_document_contents(
        &c2,
        &[
            ("/document.draw", b"document 2\n\ncontent\n"),
            ("/document-1.draw", b"document\n\ncontent 2\n"),
        ],
    )
    .await;
}

#[tokio::test]
async fn different_content_edit_mergable() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n")
        .await
        .unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document.md"]).await;
    assert::all_document_contents(&c2, &[("/document.md", b"document 2\n\ncontent 2\n")]).await;
}

#[tokio::test]
async fn different_content_edit_mergable_with_move_in_first_sync() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n")
        .await
        .unwrap();
    move_by_path(&c1, "/document.md", "/parent/").await.unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]).await;
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document 2\n\ncontent 2\n")])
        .await;
}

#[tokio::test]
async fn different_content_edit_mergable_with_move_in_second_sync() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n")
        .await
        .unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n")
        .await
        .unwrap();
    move_by_path(&c2, "/document.md", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]).await;
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document 2\n\ncontent 2\n")])
        .await;
}

#[tokio::test]
async fn move_then_edit_content() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/document.md", "/parent/").await.unwrap();
    write_path(&c2, "/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]).await;
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document content 2")]).await;
}

#[tokio::test]
async fn edit_content_then_move() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/").await.unwrap();
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document content 2")
        .await
        .unwrap();
    move_by_path(&c2, "/document.md", "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]).await;
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document content 2")]).await;
}

#[tokio::test]
async fn rename_then_edit_content() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/document.md", "document2.md")
        .await
        .unwrap();
    write_path(&c2, "/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document2.md"]).await;
    assert::all_document_contents(&c2, &[("/document2.md", b"document content 2")]).await;
}

#[tokio::test]
async fn edit_content_then_rename() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document content 2")
        .await
        .unwrap();
    rename_path(&c2, "/document.md", "document2.md")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/document2.md"]).await;
    assert::all_document_contents(&c2, &[("/document2.md", b"document content 2")]).await;
}

#[tokio::test]
async fn delete_then_edit_content() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/document.md").await.unwrap();
    write_path(&c2, "/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn edit_content_then_delete() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document.md").await.unwrap();
    write_path(&c1, "/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/document.md", b"document content 2")
        .await
        .unwrap();
    delete_path(&c2, "/document.md").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_parent_then_edit_content() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document.md").await.unwrap();
    write_path(&c1, "/parent/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/parent/").await.unwrap();
    write_path(&c2, "/parent/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn edit_content_then_delete_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document.md").await.unwrap();
    write_path(&c1, "/parent/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/parent/document.md", b"document content 2")
        .await
        .unwrap();
    delete_path(&c2, "/parent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn delete_grandparent_then_edit_content() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document.md")
        .await
        .unwrap();
    write_path(&c1, "/grandparent/parent/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&c1, "/grandparent/").await.unwrap();
    write_path(&c2, "/grandparent/parent/document.md", b"document content 2")
        .await
        .unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn edit_content_then_delete_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document.md")
        .await
        .unwrap();
    write_path(&c1, "/grandparent/parent/document.md", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/grandparent/parent/document.md", b"document content 2")
        .await
        .unwrap();
    delete_path(&c2, "/grandparent/").await.unwrap();

    sync_and_assert(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn create_two_links() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link1", document.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .create_link_at_path("/link2", document.id)
        .await
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;
    assert::all_paths(&cores[1][0], &["/", "/link1"]).await;
}

#[tokio::test]
async fn share_then_create_link_in_folder() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    let folder = cores[1][0].create_at_path("/folder/").await.unwrap();
    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .share_file(folder.id, &accounts[0].username, ShareMode::Read)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .create_link_at_path("/folder/link", document.id)
        .await
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;
    assert::all_paths(&cores[1][0], &["/", "/folder/"]).await;
}

#[tokio::test]
async fn create_link_in_folder_then_share() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    let folder = cores[1][0].create_at_path("/folder/").await.unwrap();
    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .share_file(folder.id, &accounts[0].username, ShareMode::Read)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .create_link_at_path("/folder/link", document.id)
        .await
        .unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]).await; // note: order reversed from above test
    assert::all_paths(&cores[1][0], &["/", "/folder/", "/folder/link"]).await;
}

#[tokio::test]
async fn create_link_then_move_to_owned_folder() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").await.unwrap();
    let document = cores[0][0]
        .create_at_path("/folder/document")
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .move_file(&document.id, &cores[1][1].root().await.unwrap().id)
        .await
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;

    assert::all_paths(&cores[1][0], &["/", "/link"]).await;
}

#[tokio::test]
async fn create_link_then_move_to_owned_folder_and_move_prior_parent_into_it() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let grandparent = cores[0][0].create_at_path("/grandparent/").await.unwrap();
    let parent = cores[0][0]
        .create_at_path("/grandparent/parent/")
        .await
        .unwrap();
    let folder = cores[0][0]
        .create_at_path("/grandparent/parent/child/")
        .await
        .unwrap();
    cores[0][0]
        .share_file(grandparent.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", folder.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .move_file(&folder.id, &cores[1][1].root().await.unwrap().id)
        .await
        .unwrap();
    cores[1][1].move_file(&parent.id, &folder.id).await.unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;

    assert::all_paths(&cores[1][0], &["/", "/link/"]).await;
}

#[tokio::test]
async fn create_link_then_move_to_owned_folder_and_create_file_with_conflicting_name_in_prior_parent()
 {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let grandparent = cores[0][0].create_at_path("/grandparent/").await.unwrap();
    let parent = cores[0][0]
        .create_at_path("/grandparent/parent/")
        .await
        .unwrap();
    let folder = cores[0][0]
        .create_at_path("/grandparent/parent/folder/")
        .await
        .unwrap();
    cores[0][0]
        .share_file(grandparent.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", folder.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .move_file(&folder.id, &cores[1][1].root().await.unwrap().id)
        .await
        .unwrap();
    let _new_folder = cores[1][1]
        .create_file("folder", &parent.id, FileType::Folder)
        .await
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;

    assert::all_paths(&cores[1][0], &["/", "/link/"]).await;
    assert::all_paths(
        &cores[0][0],
        &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/folder/"],
    )
    .await;
}

#[tokio::test]
async fn move_to_owned_folder_then_create_link() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").await.unwrap();
    let document = cores[0][0]
        .create_at_path("/folder/document")
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1]
        .move_file(&document.id, &cores[1][1].root().await.unwrap().id)
        .await
        .unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]).await; // note: order reversed from above test
    assert::all_paths(&cores[1][0], &["/", "/document"]).await;
}

#[tokio::test]
async fn create_link_then_delete() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").await.unwrap();
    let document = cores[0][0]
        .create_at_path("/folder/document")
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1].delete(&document.id).await.unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;

    assert::all_paths(&cores[1][0], &["/"]).await;
}

#[tokio::test]
async fn delete_then_create_link() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").await.unwrap();
    let document = cores[0][0]
        .create_at_path("/folder/document")
        .await
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1].delete(&document.id).await.unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]).await; // note: order reversed from above test

    assert::all_paths(&cores[1][0], &["/"]).await;
}

#[tokio::test]
async fn share_from_two_clients() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[0][0]).await;
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0].sync(None).await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0][1].sync(None).await.unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0][0].sync(None).await.unwrap();
    cores[0][1].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1][0].sync(None).await.unwrap();
}

#[tokio::test]
async fn share_from_two_clients_read_then_write() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[0][0]).await;
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0].sync(None).await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();

    cores[0][1].sync(None).await.unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0][0].sync(None).await.unwrap();
    cores[0][1].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1][0].sync(None).await.unwrap();
}

#[tokio::test]
async fn share_from_two_clients_write_then_read() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[0][0]).await;
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0].sync(None).await.unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0][1].sync(None).await.unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();

    cores[0][0].sync(None).await.unwrap();
    cores[0][1].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1][0].sync(None).await.unwrap();
}

#[tokio::test]
async fn share_delete_then_upgrade_to_write() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&document.id).await.unwrap();

    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();
    cores[1].sync(None).await.unwrap();
}

#[tokio::test]
async fn share_upgrade_to_write_then_delete() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1].reject_share(&document.id).await.unwrap();

    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    // note: sync order reversed from above test
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap_err();
}

#[tokio::test]
async fn deleted_share_of_file_with_local_change() {
    let mut cores = [vec![test_core_with_account().await], vec![test_core_with_account().await]];
    let new_client = another_client(&cores[1][0]).await;
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").await.unwrap();
    cores[0][0]
        .write_document(document.id, b"document content by sharer")
        .await
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0][0].sync(None).await.unwrap();

    cores[1][0].sync(None).await.unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .await
        .unwrap();

    cores[1][1].sync(None).await.unwrap();
    cores[1][1].reject_share(&document.id).await.unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]).await;

    assert::all_paths(&cores[1][0], &["/"]).await;
}
