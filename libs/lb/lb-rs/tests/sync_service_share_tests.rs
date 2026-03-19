use lb_rs::Lb;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use test_utils::*;

/// Tests that setup one device each on two accounts, share a file from one to the other, sync both, then accept
async fn assert_stuff(c1: &Lb, c2: &Lb) {
    for c in [c1, c2] {
        c.test_repo_integrity(true).await.unwrap();
        assert::local_work_paths(c, &[]).await;
        assert::server_work_paths(c, &[]).await;
        assert::deleted_files_pruned(c);
    }
    assert::all_pending_shares(c2, &[]).await;
}

#[tokio::test]
async fn new_file() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[1], &["/", "/link"]).await;
}

#[tokio::test]
async fn new_files() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").await.unwrap();
    cores[0].create_at_path("a/x/x").await.unwrap();
    let e = cores[0].create_at_path("e/").await.unwrap();
    cores[0].create_at_path("e/x/x").await.unwrap();
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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link1", shares[0].id)
        .await
        .unwrap();
    cores[1]
        .create_link_at_path("link2", shares[1].id)
        .await
        .unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    )
    .await;
}

#[tokio::test]
async fn move_file_a() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0].sync(None).await.unwrap();
    cores[0].move_file(&document.id, &folder.id).await.unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
}

#[tokio::test]
async fn create_file_in_shared_folder() {
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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    let _document = cores[1].create_at_path("/link/document").await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
}

#[tokio::test]
async fn move_file_b() {
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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();
    cores[1].move_file(&document.id, &folder.id).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]).await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]).await;
}

#[tokio::test]
async fn move_file_with_child() {
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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    let folder2 = cores[1].create_at_path("folder2/").await.unwrap();
    let document = cores[1].create_at_path("folder2/document").await.unwrap();
    cores[1]
        .write_document(document.id, b"document content")
        .await
        .unwrap();
    cores[1].move_file(&folder2.id, &folder.id).await.unwrap();
    cores[1].sync(None).await.unwrap();

    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(
        &cores[0],
        &["/", "/folder/", "/folder/folder2/", "/folder/folder2/document"],
    )
    .await;
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder2/", "/link/folder2/document"])
        .await;
    assert::all_document_contents(&cores[0], &[("/folder/folder2/document", b"document content")])
        .await;
    assert::all_document_contents(&cores[1], &[("/link/folder2/document", b"document content")])
        .await;
}

#[tokio::test]
async fn delete_accepted_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    cores[1].sync(None).await.unwrap();

    cores[1].reject_share(&folder.id).await.unwrap();
    cores[1].sync(None).await.unwrap(); // this succeeds...
    cores[1].sync(None).await.unwrap(); // ...and this fails (before being fixed)
    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[1], &["/"]).await;
}

#[tokio::test]
async fn synced_files() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").await.unwrap();
    let axx = cores[0].create_at_path("a/x/x").await.unwrap();
    let e = cores[0].create_at_path("e/").await.unwrap();
    cores[0].create_at_path("e/x/x").await.unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link1", shares[0].id)
        .await
        .unwrap();
    cores[1]
        .create_link_at_path("link2", shares[1].id)
        .await
        .unwrap();

    cores[0]
        .write_document(axx.id, b"document content")
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    )
    .await;
}

#[tokio::test]
async fn synced_files_edit_after_share() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").await.unwrap();
    let axx = cores[0].create_at_path("a/x/x").await.unwrap();
    let e = cores[0].create_at_path("e/").await.unwrap();
    cores[0].create_at_path("e/x/x").await.unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link1", shares[0].id)
        .await
        .unwrap();
    cores[1]
        .create_link_at_path("link2", shares[1].id)
        .await
        .unwrap();

    cores[0]
        .write_document(axx.id, b"document content")
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    )
    .await;
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
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[1], &["/", "/link"]).await;
    assert::all_document_contents(&cores[1], &[("/link", b"document content")]).await;
}

#[tokio::test]
async fn move_existing_edited_document_into_shared_folder() {
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

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();

    cores[1].sync(None).await.unwrap();

    cores[1].move_file(&document.id, &folder.id).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content")
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]).await;
    assert::all_document_contents(&cores[0], &[("/folder/document", b"document content")]).await;
}

#[tokio::test]
async fn create_link_in_unshared_folder() {
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

    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1].reject_share(&shares[0].id).await.unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();
    cores[1]
        .share_file(document.id, &accounts[0].username, ShareMode::Write)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    let shares = cores[0].get_pending_shares().await.unwrap();
    let _link = cores[0]
        .create_link_at_path("folder/link", shares[0].id)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/link"]).await;
    assert::all_paths(&cores[1], &["/", "/document"]).await; // NOTE: fails here; document has vanished because it's path s resolved using a shared lin.awaitk
}

#[tokio::test]
async fn move_file_out_of_shared_folder_and_delete() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    let document = cores[0].create_at_path("folder/document").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    cores[0]
        .move_file(&document.id, &roots[0].id)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();

    cores[0].delete(&document.id).await.unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    assert_stuff(&cores[0], &cores[1]).await;
    assert::all_paths(&cores[1], &["/", "/link/"]).await; // originally, failed here; deletion wasn't synced because file was no longer in user's tre.awaite
}

#[tokio::test]
async fn move_file_out_of_shared_folder_and_create_path_conflict() {
    let cores = [test_core_with_account().await, test_core_with_account().await];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = [cores[0].root().await.unwrap(), cores[1].root().await.unwrap()];

    let folder = cores[0].create_at_path("folder/").await.unwrap();
    let document = cores[0].create_at_path("folder/document").await.unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();

    cores[1].sync(None).await.unwrap();
    cores[0].sync(None).await.unwrap();

    cores[0]
        .move_file(&document.id, &roots[0].id)
        .await
        .unwrap();

    cores[0].sync(None).await.unwrap();

    cores[0].delete(&document.id).await.unwrap();
    cores[0].create_at_path("/folder/document").await.unwrap(); // originally, this would conflict with the now-moved document whose move wasn't synced to the other client

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();

    // this call now returns an error, but the code is preserved for the original reproduction of the bug
    if cores[1].get_file_by_id(document.id).await.is_ok() {
        cores[1]
            .write_document(document.id, b"document content")
            .await
            .unwrap(); // originally, this would fail with Unexpected("PathTaken")

        cores[1].sync(None).await.unwrap();

        assert_stuff(&cores[0], &cores[1]).await; // originally, if the test did make it here, validation would fail with a path conflic.awaitt
    }
}

#[tokio::test]
async fn test_share_link_write() {
    let cores = [
        test_core_with_account().await,
        test_core_with_account().await,
        test_core_with_account().await,
    ];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[2].sync(None).await.unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").await.unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]).await;
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .await
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]).await;
    assert_matches!(
        cores[1]
            .share_file(link.id, &accounts[2].username, ShareMode::Write)
            .await
            .unwrap_err()
            .kind, // this succeeded and now correctly fails (was sharing link instead of target)
        LbErrKind::InsufficientPermission
    );
    cores[1].sync(None).await.unwrap();

    cores[2].sync(None).await.unwrap(); // this failed with FileNonexistent
}

#[tokio::test]
async fn test_share_link_read() {
    let cores = [
        test_core_with_account().await,
        test_core_with_account().await,
        test_core_with_account().await,
    ];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    cores[0].sync(None).await.unwrap();
    cores[1].sync(None).await.unwrap();
    cores[2].sync(None).await.unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").await.unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync(None).await.unwrap();

    cores[1].sync(None).await.unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]).await;
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .await
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]).await;
    cores[1]
        .share_file(link.id, &accounts[2].username, ShareMode::Read)
        .await
        .unwrap();
    cores[1].sync(None).await.unwrap();

    cores[2].sync(None).await.unwrap();
}
