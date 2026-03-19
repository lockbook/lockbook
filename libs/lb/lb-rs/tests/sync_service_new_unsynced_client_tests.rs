use lb_rs::Lb;
use test_utils::*;

/// Tests that operate on one client, sync it, then create a new client without syncing (new client
/// should have no files, local work should be empty, server work should include root).
async fn assert_stuff(c: &Lb) {
    c.test_repo_integrity(true).await.unwrap();
    assert::all_paths(c, &[]).await;
    assert::all_document_contents(c, &[]).await;
    assert::local_work_paths(c, &[]).await;
}

#[tokio::test]
async fn unmodified() {
    let c1 = test_core_with_account().await;
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn new_file() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/", "/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn new_files() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/b/c/d").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn edited_document() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    write_path(&c1, "/document", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/", "/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn mv() {
    let c1 = test_core_with_account().await;
    let folder = c1.create_at_path("/folder/").await.unwrap();
    let doc = c1.create_at_path("/document").await.unwrap();
    c1.move_file(&doc.id, &folder.id).await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/", "/folder/", "/folder/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn rename() {
    let c1 = test_core_with_account().await;
    let doc = c1.create_at_path("/document").await.unwrap();
    c1.rename_file(&doc.id, "document2").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/", "/document2"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    delete_path(&c1, "/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete_parent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    delete_path(&c1, "/parent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete_grandparent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    delete_path(&c1, "/grandparent/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    assert::server_work_paths(&c2, &["/"]).await;
    assert_stuff(&c2).await;
}
