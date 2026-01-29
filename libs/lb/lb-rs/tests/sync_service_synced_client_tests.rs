use lb_rs::Lb;
use test_utils::*;

/// Tests that operate on one device and sync (work should be none, deleted files should be pruned)
async fn assert_stuff(core: &Lb) {
    core.test_repo_integrity().await.unwrap();
    assert::local_work_paths(core, &[]).await;
    assert::server_work_paths(core, &[]).await;
    assert::deleted_files_pruned(core);
    assert::new_synced_client_core_equal(core).await;
}

#[tokio::test]
async fn unmodified() {
    let core = test_core_with_account().await;
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn new_file() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn new_file_name_same_as_username() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    core.create_at_path(&format!("/{}", &account.username))
        .await
        .unwrap();
    core.sync(None).await.unwrap();
    let account = core.get_account().unwrap();
    let document_path = format!("/{}", account.username);
    assert::all_paths(&core, &["/", &document_path]).await;
    assert::all_document_contents(&core, &[(&document_path, b"")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn new_files() {
    let core = test_core_with_account().await;
    core.create_at_path("/a/b/c/d").await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]).await;
    assert::all_document_contents(&core, &[("/a/b/c/d", b"")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn edited_document() {
    let core = test_core_with_account().await;
    core.create_at_path("/document").await.unwrap();
    write_path(&core, "/document", b"document content")
        .await
        .unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/", "/document"]).await;
    assert::all_document_contents(&core, &[("/document", b"document content")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn mv() {
    let core = test_core_with_account().await;
    let folder = core.create_at_path("/folder/").await.unwrap();
    let doc = core.create_at_path("/document").await.unwrap();
    core.move_file(&doc.id, &folder.id).await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/", "/folder/", "/folder/document"]).await;
    assert::all_document_contents(&core, &[("/folder/document", b"")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn rename() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.rename_file(&doc.id, "document2").await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/", "/document2"]).await;
    assert::all_document_contents(&core, &[("/document2", b"")]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn delete() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/document").await.unwrap();
    core.delete(&doc.id).await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn delete_parent() {
    let core = test_core_with_account().await;
    core.create_at_path("/folder/document").await.unwrap();
    delete_path(&core, "/folder/").await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert_stuff(&core).await;
}

#[tokio::test]
async fn delete_grandparent() {
    let core = test_core_with_account().await;
    core.create_at_path("/grandparent/parent/document")
        .await
        .unwrap();
    delete_path(&core, "/grandparent/").await.unwrap();
    core.sync(None).await.unwrap();
    assert::all_paths(&core, &["/"]).await;
    assert::all_document_contents(&core, &[]).await;
    assert_stuff(&core).await;
}
