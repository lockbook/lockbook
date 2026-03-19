use lb_rs::Lb;
use test_utils::*;

/// Tests which are constructed to test cycle resolution. These are tests that setup two synced
/// devices, operate on both devices, then sync both twice (work should be none, devices dbs should
/// be equal, deleted files should be pruned)
async fn sync_and_assert_stuff(c1: &Lb, c2: &Lb) {
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
async fn two_cycle() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c/", "/c/b/", "/c/b/a/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/c/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/d/", "/d/c/", "/d/c/b/", "/d/c/b/a/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c/", "/c/b/", "/c/b/a/", "/d/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/d/", "/d/c/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/c/", "/d/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn two_cycle_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c1, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/c2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();
    rename_path(&c1, "/d/", "d2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c1, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c1, "/c2/", "/d2/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();
    rename_path(&c1, "/d/", "d2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c1, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();
    rename_path(&c1, "/d/", "d2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c2/", "/d2/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted_with_renames_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    rename_path(&c1, "/a/", "a2").await.unwrap();
    rename_path(&c1, "/b/", "b2").await.unwrap();
    rename_path(&c1, "/c/", "c2").await.unwrap();
    rename_path(&c1, "/d/", "d2").await.unwrap();

    move_by_path(&c1, "/a2/", "/b2/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn two_cycle_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    move_by_path(&c2, "/b2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    move_by_path(&c2, "/c2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    move_by_path(&c2, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c2, "/c2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/c2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();

    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    rename_path(&c2, "/d/", "d2").await.unwrap();

    move_by_path(&c2, "/d2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();

    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    rename_path(&c2, "/d/", "d2").await.unwrap();

    move_by_path(&c2, "/c2/", "/d2/").await.unwrap();
    move_by_path(&c2, "/d2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();

    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    rename_path(&c2, "/d/", "d2").await.unwrap();

    move_by_path(&c2, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted_with_renames_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();

    rename_path(&c2, "/a/", "a2").await.unwrap();
    rename_path(&c2, "/b/", "b2").await.unwrap();
    rename_path(&c2, "/c/", "c2").await.unwrap();
    rename_path(&c2, "/d/", "d2").await.unwrap();

    move_by_path(&c2, "/b2/", "/c2/").await.unwrap();
    move_by_path(&c2, "/c2/", "/d2/").await.unwrap();
    move_by_path(&c2, "/d2/", "/a2/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn two_cycle_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();
    delete_path(&c1, "/b/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();
    delete_path(&c1, "/c/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();
    delete_path(&c1, "/b/").await.unwrap();
    delete_path(&c1, "/c/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();
    delete_path(&c1, "/d/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();
    delete_path(&c1, "/c/").await.unwrap();
    delete_path(&c1, "/d/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();
    delete_path(&c1, "/b/").await.unwrap();
    delete_path(&c1, "/d/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted_with_deletes_first_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    delete_path(&c1, "/b/").await.unwrap();
    delete_path(&c1, "/c/").await.unwrap();
    delete_path(&c1, "/d/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn two_cycle_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();
    delete_path(&c2, "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();
    delete_path(&c2, "/a/").await.unwrap();
    delete_path(&c2, "/b/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();
    delete_path(&c2, "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    delete_path(&c2, "/a/").await.unwrap();
    delete_path(&c2, "/b/").await.unwrap();
    delete_path(&c2, "/c/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    delete_path(&c2, "/a/").await.unwrap();
    delete_path(&c2, "/b/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    delete_path(&c2, "/a/").await.unwrap();
    delete_path(&c2, "/c/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted_with_deletes_second_device() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.create_at_path("/c/").await.unwrap();
    c1.create_at_path("/d/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();
    delete_path(&c2, "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn move_two_cycle_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/b/child/", "/b/a/child/"]).await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn move_two_cycle_with_modified_document_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child").await.unwrap();
    c1.create_at_path("/b/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    write_path(&c1, "/a/child", b"document content")
        .await
        .unwrap();
    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/b/a/child"]).await;
    assert::all_document_contents(&c2, &[("/b/a/child", b"document content")]).await;
}

#[tokio::test]
async fn three_cycle_one_move_reverted_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &["/", "/c/", "/c/b/", "/c/b/a/", "/c/child/", "/c/b/child/", "/c/b/a/child/"],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn three_cycle_two_moves_reverted_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(&c2, &["/", "/b/", "/b/a/", "/c/", "/b/child/", "/b/a/child/", "/c/child/"])
        .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_one_move_reverted_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.create_at_path("/d/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &[
            "/",
            "/d/",
            "/d/c/",
            "/d/c/b/",
            "/d/c/b/a/",
            "/d/child/",
            "/d/c/child/",
            "/d/c/b/child/",
            "/d/c/b/a/child/",
        ],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_adjacent_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.create_at_path("/d/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c1, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &[
            "/",
            "/c/",
            "/c/b/",
            "/c/b/a/",
            "/d/",
            "/c/child/",
            "/c/b/child/",
            "/c/b/a/child/",
            "/d/child/",
        ],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_two_moves_reverted_alternating_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.create_at_path("/d/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c1, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &[
            "/",
            "/b/",
            "/b/a/",
            "/d/",
            "/d/c/",
            "/b/child/",
            "/b/a/child/",
            "/d/child/",
            "/d/c/child/",
        ],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}

#[tokio::test]
async fn four_cycle_three_moves_reverted_with_children() {
    let c1 = test_core_with_account().await;
    c1.create_at_path("/a/child/").await.unwrap();
    c1.create_at_path("/b/child/").await.unwrap();
    c1.create_at_path("/c/child/").await.unwrap();
    c1.create_at_path("/d/child/").await.unwrap();
    c1.sync(None).await.unwrap();

    let c2 = another_client(&c1).await;
    c2.sync(None).await.unwrap();

    move_by_path(&c1, "/a/", "/b/").await.unwrap();
    move_by_path(&c2, "/b/", "/c/").await.unwrap();
    move_by_path(&c2, "/c/", "/d/").await.unwrap();
    move_by_path(&c2, "/d/", "/a/").await.unwrap();

    sync_and_assert_stuff(&c1, &c2).await;
    assert::all_paths(
        &c2,
        &["/", "/b/", "/b/a/", "/c/", "/d/", "/b/child/", "/b/a/child/", "/c/child/", "/d/child/"],
    )
    .await;
    assert::all_document_contents(&c2, &[]).await;
}
