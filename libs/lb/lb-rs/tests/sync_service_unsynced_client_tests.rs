use test_utils::*;

/// Tests that operate on one client without syncing.

#[tokio::test]
async fn unmodified() {
    let core = test_core_with_account().await;
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert::local_work_paths(&core, &[]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn new_file() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert::local_work_paths(&core, &["/document"]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn new_files() {
    let core = test_core_with_account().await;
    core.create_at_path("/a/b/c/d").await.unwrap();
    assert::all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert::all_document_contents(&core, &[("/a/b/c/d", b"")]).await;
    assert::local_work_paths(&core, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn edited_document() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    write_path(&core, "/document", b"document content")
        .await
        .unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"document content")]).await;
    assert::local_work_paths(&core, &["/document"]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn mv() {
    let core = test_core_with_account().await;
    let new_parent = core.create_at_path("/folder/").await.unwrap();
    let doc = core.create_at_path("/document").await.unwrap();
    core.move_file(&doc.id, &new_parent.id).await.unwrap();
    assert::all_paths(&core, &["/", "/folder/", "/folder/document"]).await;
    assert::all_document_contents(&core, &[("/folder/document", b"")]).await;
    assert::local_work_paths(&core, &["/folder/", "/folder/document"]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn rename() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.rename_file(&doc.id, "document2").await.unwrap();
    assert::all_paths(&core, &["/", "/document2"]).await;
    assert::all_document_contents(&core, &[("/document2", b"")]).await;
    assert::local_work_paths(&core, &["/document2"]).await;
    assert::server_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
}

// the idea of the next three tests is to assert that new+deleted content isn't sent to the server
// changes to sync made it cumbersome to detect this without actually performing the sync.
// but those changes did make it easy to determine what happened during this sync. So these tests
// will sync (even though the filename suggests they shouldn't), and will assert that nothing was
// sent
#[tokio::test]
async fn delete() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.delete(&doc.id).await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    let summary = core.sync(None).await.unwrap();
    assert!(summary.work_units.is_empty());
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn delete_parent() {
    let core = test_core_with_account().await;
    core.create_at_path("/parent/document").await.unwrap();
    delete_path(&core, "/parent/").await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    let summary = core.sync(None).await.unwrap();
    assert!(summary.work_units.is_empty());
    core.test_repo_integrity(true).await.unwrap();
}

#[tokio::test]
async fn delete_grandparent() {
    let core = test_core_with_account().await;
    core.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    delete_path(&core, "/grandparent/").await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    let summary = core.sync(None).await.unwrap();
    assert!(summary.work_units.is_empty());
    core.test_repo_integrity(true).await.unwrap();
}
