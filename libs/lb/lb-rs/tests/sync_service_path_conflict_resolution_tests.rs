use lb_rs::Lb;
use test_utils::*;

/// Tests which are constructed to test path conflict resolution Like those above, these are tests
/// that setup two synced clients, operate on both clients, then sync both twice (work should be
/// none, client dbs should be equal, deleted files should be pruned).
async fn sync_and_assert_stuff(c1: &Lb, c2: &Lb) {
    c1.sync(None).await.unwrap();
    c2.sync(None).await.unwrap();
    c1.sync(None).await.unwrap();
    c2.sync(None).await.unwrap();

    c1.test_repo_integrity().await.unwrap();
    assert::cores_equal(c1, c2).await;
    assert::local_work_paths(c1, &[]).await;
    assert::server_work_paths(c1, &[]).await;
    assert::deleted_files_pruned(c1);
}

#[tokio::test]
async fn concurrent_create_documents() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a.md").await.unwrap();
    c2.create_at_path("/a.md").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md"]).await;
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_create_folders() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a/").await.unwrap();
    c2.create_at_path("/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a/", "/a-1/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn concurrent_create_folders_with_children() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a/child/").await.unwrap();
    c2.create_at_path("/a/child/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a/", "/a-1/", "/a/child/", "/a-1/child/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn concurrent_create_document_then_folder() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a.md").await.unwrap();
    c2.create_at_path("/a.md/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md/"]).await;
    assert::all_document_contents(&c2, &[("/a.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_create_folder_then_document() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a.md/").await.unwrap();
    c2.create_at_path("/a.md").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a.md/", "/a-1.md"]).await;
    assert::all_document_contents(&c2, &[("/a-1.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_create_document_then_folder_with_child() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a.md").await.unwrap();
    c2.create_at_path("/a.md/child/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md/", "/a-1.md/child/"]).await;
    assert::all_document_contents(&c2, &[("/a.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_create_folder_with_child_then_document() {
    let c1 = test_core_with_account().await;
    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();
    c1.create_at_path("/a.md/child/").await.unwrap();
    c2.create_at_path("/a.md").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/a.md/", "/a.md/child/", "/a-1.md"]).await;
    assert::all_document_contents(&c2, &[("/a-1.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_move_then_create_documents() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/folder/a.md", "").await.unwrap();
    c2.create_at_path("/a.md").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]).await;
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_create_then_move_documents() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/a.md").await.unwrap();
    move_by_path(&c2, "/folder/a.md", "").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]).await;
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]).await;
}

#[tokio::test]
async fn concurrent_move_then_create_folders() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/folder/a.md/", "").await.unwrap();
    c2.create_at_path("/a.md/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn concurrent_create_then_move_folders() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/a.md/").await.unwrap();
    move_by_path(&c2, "/folder/a.md/", "").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn concurrent_move_then_create_folders_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/folder/a.md/", "").await.unwrap();
    c2.create_at_path("/a.md/child/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn concurrent_create_then_move_folders_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/folder/a.md/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    c1.create_at_path("/a.md/child/").await.unwrap();
    move_by_path(&c2, "/folder/a.md/", "").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}
