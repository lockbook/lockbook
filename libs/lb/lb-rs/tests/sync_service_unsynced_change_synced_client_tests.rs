use test_utils::*;

/// Tests that operate on one device after syncing.

#[tokio::test]
async fn new_file() {
    let core = test_core_with_account().await;
    core.sync(None).await.unwrap();
    core.create_at_path("/document").await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert::local_work_paths(&core, &["/document"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn new_files() {
    let core = test_core_with_account().await;
    core.sync(None).await.unwrap();
    core.create_at_path("/a/b/c/d").await.unwrap();
    assert::all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert::all_document_contents(&core, &[("/a/b/c/d", b"")]).await;
    assert::local_work_paths(&core, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn edited_document() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    core.sync(None).await.unwrap();
    write_path(&core, "/document", b"document content")
        .await
        .unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"document content")]).await;
    assert::local_work_paths(&core, &["/document"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn edit_unedit() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    write_path(&core, "/document", b"").await.unwrap();
    core.sync(None).await.unwrap();
    write_path(&core, "/document", b"document content")
        .await
        .unwrap();
    write_path(&core, "/document", b"").await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert::local_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn mv() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    let folder = core.create_at_path("/folder/").await.unwrap();
    core.sync(None).await.unwrap();
    core.move_file(&doc.id, &folder.id).await.unwrap();
    assert::all_paths(&core, &["/", "/folder/", "/folder/document"]).await;
    assert::all_document_contents(&core, &[("/folder/document", b"")]).await;
    assert::local_work_paths(&core, &["/folder/document"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn move_unmove() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    let folder = core.create_at_path("/folder/").await.unwrap();
    core.sync(None).await.unwrap();
    core.move_file(&doc.id, &folder.id).await.unwrap();
    core.move_file(&doc.id, &core.root().await.unwrap().id)
        .await
        .unwrap();
    assert::all_paths(&core, &["/", "/folder/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert::local_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn rename() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.sync(None).await.unwrap();
    core.rename_file(&doc.id, "document2").await.unwrap();
    assert::all_paths(&core, &["/", "/document2"]).await;
    assert::all_document_contents(&core, &[("/document2", b"")]).await;
    assert::local_work_paths(&core, &["/document2"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn rename_unrename() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.sync(None).await.unwrap();
    core.rename_file(&doc.id, "document2").await.unwrap();
    core.rename_file(&doc.id, "document").await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert::local_work_paths(&core, &[]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn delete() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    core.sync(None).await.unwrap();
    delete_path(&core, "/document").await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert::local_work_paths(&core, &["/document"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn delete_parent() {
    let core = test_core_with_account().await;
    core.create_at_path("/parent/document").await.unwrap();
    core.sync(None).await.unwrap();
    delete_path(&core, "/parent/").await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert::local_work_paths(&core, &["/parent/"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}

#[tokio::test]
async fn delete_grandparent() {
    let core = test_core_with_account().await;
    core.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    core.sync(None).await.unwrap();
    delete_path(&core, "/grandparent/").await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert::local_work_paths(&core, &["/grandparent/"]).await;
    core.test_repo_integrity(true).await.unwrap();
    assert::server_work_paths(&core, &[]).await;
}
