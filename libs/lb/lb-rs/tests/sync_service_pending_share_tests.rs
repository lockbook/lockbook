use itertools::Itertools;
use lb_rs::Lb;
use lb_rs::model::file::ShareMode;
use test_utils::*;

/// Tests that setup one device each on two accounts, share a file from one to the other, then sync both
async fn assert_stuff(c1: &Lb, c2: &Lb) {
    for c in [c1, c2] {
        c.test_repo_integrity().await.unwrap();
        assert::local_work_paths(c, &[]).await;
        assert::server_work_paths(c, &[]).await;
        assert::deleted_files_pruned(c);
    }
    assert::all_paths(c2, &["/"]).await;
    assert::all_document_contents(c2, &[]).await;
}

#[tokio::test]
async fn new_file() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_pending_shares(&cores[1], &["folder"]).await;
}

#[tokio::test]
async fn new_files() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").await.unwrap();
    cores[0].create_at_path("a/b/c/d").await.unwrap();
    let e = cores[0].create_at_path("e/").await.unwrap();
    cores[0].create_at_path("e/f/g/h").await.unwrap();
    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_pending_shares(&cores[1], &["a", "e"]).await;

    // pending descendants tests
    let mut names = cores[1]
        .get_pending_share_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect_vec();
    names.sort();
    assert_eq!(names, ["a", "b", "c", "d", "e", "f", "g", "h"]);

    cores[0]
        .delete(&cores[0].get_by_path("a/b/c").await.unwrap().id)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let mut names = cores[1]
        .get_pending_share_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect_vec();
    names.sort();
    assert_eq!(names, ["a", "b", "e", "f", "g", "h"]);
}

#[tokio::test]
async fn edited_document() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_pending_shares(&cores[1], &["document"]).await;
}

#[tokio::test]
async fn preview_pending_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0]
        .write_document(document.id, b"document content")
        .await
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert_eq!(cores[1].read_document(document.id, false).await.unwrap(), b"document content");
}
