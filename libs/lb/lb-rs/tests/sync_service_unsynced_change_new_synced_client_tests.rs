use lb_rs::Lb;
use test_utils::*;

/// Tests that setup two synced clients, operate on one client, and sync it without syncing the
/// other client.

async fn assert_stuff(c: &Lb) {
    c.test_repo_integrity().await.unwrap();
    assert::local_work_paths(c, &[]).await;
}

#[tokio::test]
async fn unmodified() {
    let mut c1 = test_core_with_account().await;
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
    assert::server_work_paths(&c2, &[]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn new_file() {
    let mut c1 = test_core_with_account().await;

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
    assert::server_work_paths(&c2, &["/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn new_files() {
    let mut c1 = test_core_with_account().await;

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/a/b/c/d").await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
    assert::server_work_paths(&c2, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn edited_document() {
    let mut c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    write_path(&mut c1, "/document", b"document content")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/", "/document"]).await;
    assert::all_document_contents(&c2, &[("/document", b"")]).await;
    assert::server_work_paths(&c2, &["/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn mv() {
    let mut c1 = test_core_with_account().await;
    let folder = c1.create_at_path("/folder/").await.unwrap();
    let doc = c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    c1.move_file(&doc.id, &folder.id).await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/", "/folder/", "/document"]).await;
    assert::all_document_contents(&c2, &[("/document", b"")]).await;
    assert::server_work_paths(&c2, &["/folder/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn rename() {
    let mut c1 = test_core_with_account().await;
    let doc = c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    c1.rename_file(&doc.id, "document2").await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/", "/document"]).await;
    assert::all_document_contents(&c2, &[("/document", b"")]).await;
    assert::server_work_paths(&c2, &["/document2"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete() {
    let mut c1 = test_core_with_account().await;
    c1.create_at_path("/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&mut c1, "/document").await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(&c2, &["/", "/document"]).await;
    assert::all_document_contents(&c2, &[("/document", b"")]).await;
    assert::server_work_paths(&c2, &["/document"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete_parent() {
    let mut c1 = test_core_with_account().await;
    c1.create_at_path("/parent/document").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&mut c1, "/parent/").await.unwrap();
    c1.sync(None).await.unwrap();
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document"]).await;
    assert::all_document_contents(&c2, &[("/parent/document", b"")]).await;
    assert::server_work_paths(&c2, &["/parent/"]).await;
    assert_stuff(&c2).await;
}

#[tokio::test]
async fn delete_grandparent() {
    let mut c1 = test_core_with_account().await;
    c1.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&mut c1).await;
    c2.sync(None).await.unwrap();

    delete_path(&mut c1, "/grandparent/").await.unwrap();
    c1.sync(None).await.unwrap();

    assert::all_paths(
        &c2,
        &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
    )
    .await;
    assert::all_document_contents(&c2, &[("/grandparent/parent/document", b"")]).await;
    assert::server_work_paths(&c2, &["/grandparent/"]).await;
    assert_stuff(&c2).await;
}
