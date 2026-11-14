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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();

    let document = cores[0].create_at_path("document").await.unwrap();
    cores[0].sync().await.unwrap();
    cores[0].move_file(&document.id, &folder.id).await.unwrap();
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();

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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    let _document = cores[1].create_at_path("/link/document").await.unwrap();
    cores[1].sync().await.unwrap();
    cores[0].sync().await.unwrap();
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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();
    cores[1].move_file(&document.id, &folder.id).await.unwrap();
    cores[1].sync().await.unwrap();

    cores[0].sync().await.unwrap();

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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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
    cores[1].sync().await.unwrap();

    cores[0].sync().await.unwrap();

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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1]
        .create_link_at_path("link", shares[0].id)
        .await
        .unwrap();
    cores[1].sync().await.unwrap();

    cores[1].reject_share(&folder.id).await.unwrap();
    cores[1].sync().await.unwrap(); // this succeeds...
    cores[1].sync().await.unwrap(); // ...and this fails (before being fixed)
    cores[0].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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
    cores[0].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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
    cores[0].sync().await.unwrap();

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
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();

    cores[1].sync().await.unwrap();

    cores[1].move_file(&document.id, &folder.id).await.unwrap();
    cores[1]
        .write_document(document.id, b"document content")
        .await
        .unwrap();

    cores[1].sync().await.unwrap();
    cores[0].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    cores[1].reject_share(&shares[0].id).await.unwrap();
    let document = cores[1].create_at_path("document").await.unwrap();
    cores[1]
        .share_file(document.id, &accounts[0].username, ShareMode::Write)
        .await
        .unwrap();

    cores[1].sync().await.unwrap();
    cores[0].sync().await.unwrap();

    let shares = cores[0].get_pending_shares().await.unwrap();
    let _link = cores[0]
        .create_link_at_path("folder/link", shares[0].id)
        .await
        .unwrap();

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();

    cores[1].sync().await.unwrap();
    cores[0].sync().await.unwrap();

    cores[0]
        .move_file(&document.id, &roots[0].id)
        .await
        .unwrap();

    cores[0].sync().await.unwrap();

    cores[0].delete(&document.id).await.unwrap();

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    let shares = cores[1].get_pending_shares().await.unwrap();
    let _link = cores[1]
        .create_link_at_path("link/", shares[0].id)
        .await
        .unwrap();

    cores[1].sync().await.unwrap();
    cores[0].sync().await.unwrap();

    cores[0]
        .move_file(&document.id, &roots[0].id)
        .await
        .unwrap();

    cores[0].sync().await.unwrap();

    cores[0].delete(&document.id).await.unwrap();
    cores[0].create_at_path("/folder/document").await.unwrap(); // originally, this would conflict with the now-moved document whose move wasn't synced to the other client

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();

    // this call now returns an error, but the code is preserved for the original reproduction of the bug
    if cores[1].get_file_by_id(document.id).await.is_ok() {
        cores[1]
            .write_document(document.id, b"document content")
            .await
            .unwrap(); // originally, this would fail with Unexpected("PathTaken")

        cores[1].sync().await.unwrap();

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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();
    cores[2].sync().await.unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").await.unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Write)
        .await
        .unwrap();
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]).await;
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .await
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]).await;
    cores[1]
        .share_file(link.id, &accounts[2].username, ShareMode::Write)
        .await
        .unwrap(); // shares the target, not the link
    cores[1].sync().await.unwrap();

    cores[2].sync().await.unwrap(); // this failed with FileNonexistent
    assert::all_pending_shares(&cores[2], &["passalong.md"]).await;
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

    cores[0].sync().await.unwrap();
    cores[1].sync().await.unwrap();
    cores[2].sync().await.unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").await.unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Read)
        .await
        .unwrap();
    cores[0].sync().await.unwrap();

    cores[1].sync().await.unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]).await;
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .await
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]).await;
    assert_matches!(
        cores[1]
            .share_file(link.id, &accounts[2].username, ShareMode::Read)
            .await
            .unwrap_err()
            .kind,
        LbErrKind::InsufficientPermission
    );
}

#[tokio::test]
async fn downgrade_write_to_read_survives_owners_sync() {
    let a = test_core_with_account().await;
    let b = test_core_with_account().await;
    let acct_b = b.get_account().unwrap().clone();

    let doc = a.create_at_path("/doc.md").await.unwrap();
    a.share_file(doc.id, &acct_b.username, ShareMode::Write)
        .await
        .unwrap();
    a.sync().await.unwrap();

    a.share_file(doc.id, &acct_b.username, ShareMode::Read)
        .await
        .unwrap();
    a.sync().await.unwrap();

    let shares = a.get_file_by_id(doc.id).await.unwrap().shares;
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].mode, ShareMode::Read);
}

#[tokio::test]
async fn rejected_share_survives_owners_concurrent_rename() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let doc = owner.create_at_path("/doc.md").await.unwrap();
    owner
        .share_file(doc.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();

    recipient.reject_share(&doc.id).await.unwrap();
    recipient.sync().await.unwrap();

    owner.rename_file(&doc.id, "renamed.md").await.unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();

    assert::all_pending_shares(&recipient, &[]).await;
}

/// A deleted file keeps its owner when a collaborator moves its folder out of the share; the owner
/// must still be able to push afterwards.
#[tokio::test]
async fn collaborator_moves_out_folder_holding_owners_deleted_file() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let project = owner.create_at_path("/project/").await.unwrap();
    let design = owner.create_at_path("/project/design/").await.unwrap();
    let mockup = owner
        .create_at_path("/project/design/mockup.png")
        .await
        .unwrap();
    owner.sync().await.unwrap();
    owner.delete(&mockup.id).await.unwrap();
    owner
        .share_file(project.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();

    collaborator.sync().await.unwrap();
    let collaborator_root = collaborator.root().await.unwrap().id;
    collaborator
        .move_file(&design.id, &collaborator_root)
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    owner.create_at_path("/notes.md").await.unwrap();
    owner.sync().await.unwrap();
}

/// Same with a deleted folder that still has files inside it.
#[tokio::test]
async fn collaborator_moves_out_folder_holding_owners_deleted_folder() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let project = owner.create_at_path("/project/").await.unwrap();
    let design = owner.create_at_path("/project/design/").await.unwrap();
    let old = owner.create_at_path("/project/design/old/").await.unwrap();
    owner
        .create_at_path("/project/design/old/mockup.png")
        .await
        .unwrap();
    owner.sync().await.unwrap();
    owner.delete(&old.id).await.unwrap();
    owner
        .share_file(project.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();

    collaborator.sync().await.unwrap();
    let collaborator_root = collaborator.root().await.unwrap().id;
    collaborator
        .move_file(&design.id, &collaborator_root)
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    owner.create_at_path("/notes.md").await.unwrap();
    owner.sync().await.unwrap();
}

/// Same, seen from another owner device that last synced before the file inside the deleted folder
/// changed.
#[tokio::test]
async fn collaborator_moves_out_folder_holding_owners_edited_deleted_folder() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let project = owner.create_at_path("/project/").await.unwrap();
    let design = owner.create_at_path("/project/design/").await.unwrap();
    let old = owner.create_at_path("/project/design/old/").await.unwrap();
    let mockup = owner
        .create_at_path("/project/design/old/mockup.png")
        .await
        .unwrap();
    owner.sync().await.unwrap();
    let other_device = another_client(&owner).await;
    other_device.sync().await.unwrap();

    owner.write_document(mockup.id, b"mockup").await.unwrap();
    owner.sync().await.unwrap();
    owner.delete(&old.id).await.unwrap();
    owner
        .share_file(project.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();

    collaborator.sync().await.unwrap();
    let collaborator_root = collaborator.root().await.unwrap().id;
    collaborator
        .move_file(&design.id, &collaborator_root)
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    other_device.sync().await.unwrap();
    assert::all_paths(&other_device, &["/", "/project/"]).await;
}

/// Declining a share with an unsynced edit inside it drops the edit rather than wedging sync.
#[tokio::test]
async fn reject_share_with_unsynced_create_inside_it() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let folder = owner.create_at_path("/folder/").await.unwrap();
    owner
        .share_file(folder.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();

    recipient
        .create_file("notes.md", &folder.id, lb_rs::model::file_metadata::FileType::Document)
        .await
        .unwrap();
    recipient.reject_share(&folder.id).await.unwrap();

    recipient.sync().await.unwrap();
    recipient.sync().await.unwrap();
    assert::all_paths(&recipient, &["/"]).await;
    assert::all_pending_shares(&recipient, &[]).await;

    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/folder/"]).await;
}

/// Deleting a note and then moving its folder into someone else's share syncs both changes.
#[tokio::test]
async fn delete_then_move_folder_into_share() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    collaborator.sync().await.unwrap();
    collaborator
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    collaborator.create_at_path("/notes/old.md").await.unwrap();
    collaborator.sync().await.unwrap();

    delete_path(&collaborator, "/notes/old.md").await.unwrap();
    move_by_path(&collaborator, "/notes/", "/team/")
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/team/", "/team/notes/"]).await;
}

/// Moving a folder out into someone else's share and then deleting the folder it left syncs both
/// changes.
#[tokio::test]
async fn move_folder_into_share_then_delete_its_old_parent() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    collaborator.sync().await.unwrap();
    collaborator
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    collaborator
        .create_at_path("/old/notes/drafts/")
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    move_by_path(&collaborator, "/old/notes/", "/team/")
        .await
        .unwrap();
    delete_path(&collaborator, "/old/").await.unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/team/", "/team/notes/", "/team/notes/drafts/"]).await;
}

/// A note moved into a folder that is then deleted stays deleted when its old folder moves into
/// someone else's share.
#[tokio::test]
async fn move_note_into_deleted_folder_then_move_its_old_folder_into_share() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    collaborator.sync().await.unwrap();
    collaborator
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    collaborator.create_at_path("/notes/old.md").await.unwrap();
    collaborator.create_at_path("/trash/").await.unwrap();
    collaborator.sync().await.unwrap();

    move_by_path(&collaborator, "/notes/old.md", "/trash/")
        .await
        .unwrap();
    delete_path(&collaborator, "/trash/").await.unwrap();
    move_by_path(&collaborator, "/notes/", "/team/")
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/team/", "/team/notes/"]).await;
}

/// A deleted note stays deleted when its folder moves out of a deleted folder into someone else's
/// share.
#[tokio::test]
async fn delete_note_then_move_its_folder_out_of_a_deleted_one_into_share() {
    // the merge replays files in hash order, so each round tries another order
    for _ in 0..20 {
        let owner = test_core_with_account().await;
        let collaborator = test_core_with_account().await;
        let collaborator_name = collaborator.get_account().unwrap().username.clone();

        let team = owner.create_at_path("/team/").await.unwrap();
        owner
            .share_file(team.id, &collaborator_name, ShareMode::Write)
            .await
            .unwrap();
        owner.sync().await.unwrap();
        collaborator.sync().await.unwrap();
        collaborator
            .create_link_at_path("/team", team.id)
            .await
            .unwrap();
        collaborator
            .create_at_path("/old/notes/old.md")
            .await
            .unwrap();
        collaborator.sync().await.unwrap();

        delete_path(&collaborator, "/old/notes/old.md")
            .await
            .unwrap();
        move_by_path(&collaborator, "/old/notes/", "/team/")
            .await
            .unwrap();
        delete_path(&collaborator, "/old/").await.unwrap();
        collaborator.sync().await.unwrap();

        owner.sync().await.unwrap();
        assert::all_paths(&owner, &["/", "/team/", "/team/notes/"]).await;
    }
}

/// A collaborator who moves a folder out of a share and then leaves it keeps the folder, and a
/// file shared from inside it still syncs.
#[tokio::test]
async fn move_folder_out_of_share_then_leave_and_share_inside() {
    let owner = test_core_with_account().await;
    let friend = test_core_with_account().await;
    let other = test_core_with_account().await;
    let friend_name = friend.get_account().unwrap().username.clone();
    let other_name = other.get_account().unwrap().username.clone();

    let trip = owner.create_at_path("/trip/").await.unwrap();
    let flights = owner.create_at_path("/trip/flights/").await.unwrap();
    owner
        .share_file(trip.id, &friend_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    friend.sync().await.unwrap();
    friend.create_link_at_path("/trip", trip.id).await.unwrap();
    friend.sync().await.unwrap();

    let friend_root = friend.root().await.unwrap().id;
    friend.move_file(&flights.id, &friend_root).await.unwrap();
    friend.reject_share(&trip.id).await.unwrap();
    let note = friend.create_at_path("/flights/note.md").await.unwrap();
    friend
        .share_file(note.id, &other_name, ShareMode::Read)
        .await
        .unwrap();

    friend.sync().await.unwrap();
    friend.sync().await.unwrap();
    assert::all_paths(&friend, &["/", "/flights/", "/flights/note.md"]).await;
    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/trip/"]).await;
}

/// Moves on two accounts that form a cycle inside a share are undone even when the collaborator
/// also shares a file inside the cycle.
#[tokio::test]
async fn move_cycle_in_share_with_share_inside() {
    let owner = test_core_with_account().await;
    let friend = test_core_with_account().await;
    let other = test_core_with_account().await;
    let friend_name = friend.get_account().unwrap().username.clone();
    let other_name = other.get_account().unwrap().username.clone();

    let trip = owner.create_at_path("/trip/").await.unwrap();
    let a = owner.create_at_path("/trip/a/").await.unwrap();
    let b = owner.create_at_path("/trip/b/").await.unwrap();
    let doc = owner.create_at_path("/trip/a/doc.md").await.unwrap();
    owner
        .share_file(trip.id, &friend_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    friend.sync().await.unwrap();
    friend.create_link_at_path("/trip", trip.id).await.unwrap();
    friend.sync().await.unwrap();

    owner.move_file(&b.id, &a.id).await.unwrap();
    owner.sync().await.unwrap();
    friend.move_file(&a.id, &b.id).await.unwrap();
    friend
        .share_file(doc.id, &other_name, ShareMode::Read)
        .await
        .unwrap();

    friend.sync().await.unwrap();
    assert::all_paths(&friend, &["/", "/trip/", "/trip/a/", "/trip/a/b/", "/trip/a/doc.md"]).await;
}

/// A device that hasn't seen a folder move into the recipient's own share can still share it with
/// them; once it syncs, the share is moot and sync goes on.
#[tokio::test]
async fn share_with_new_owner_of_moved_folder() {
    let alice = test_core_with_account().await;
    let bob = test_core_with_account().await;
    let carol = test_core_with_account().await;
    let alice_name = alice.get_account().unwrap().username.clone();
    let bob_name = bob.get_account().unwrap().username.clone();
    let carol_name = carol.get_account().unwrap().username.clone();

    let team = alice.create_at_path("/team/").await.unwrap();
    alice
        .share_file(team.id, &bob_name, ShareMode::Write)
        .await
        .unwrap();
    alice.sync().await.unwrap();
    bob.sync().await.unwrap();
    bob.create_link_at_path("/team", team.id).await.unwrap();

    let drafts = bob.create_at_path("/drafts/").await.unwrap();
    bob.share_file(drafts.id, &carol_name, ShareMode::Write)
        .await
        .unwrap();
    bob.sync().await.unwrap();
    carol.sync().await.unwrap();
    carol
        .create_link_at_path("/drafts", drafts.id)
        .await
        .unwrap();
    carol.sync().await.unwrap();

    move_by_path(&bob, "/drafts/", "/team/").await.unwrap();
    bob.sync().await.unwrap();

    carol
        .share_file(drafts.id, &alice_name, ShareMode::Read)
        .await
        .unwrap();
    carol.sync().await.unwrap();
    carol.sync().await.unwrap();
    assert::all_paths(&carol, &["/", "/drafts/"]).await;
}

/// Sharing a note with someone and then moving its folder into their share syncs the move.
#[tokio::test]
async fn share_note_then_move_its_folder_into_recipients_share() {
    let owner = test_core_with_account().await;
    let collaborator = test_core_with_account().await;
    let owner_name = owner.get_account().unwrap().username.clone();
    let collaborator_name = collaborator.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &collaborator_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    collaborator.sync().await.unwrap();
    collaborator
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    let note = collaborator.create_at_path("/notes/note.md").await.unwrap();
    collaborator.sync().await.unwrap();

    collaborator
        .share_file(note.id, &owner_name, ShareMode::Read)
        .await
        .unwrap();
    move_by_path(&collaborator, "/notes/", "/team/")
        .await
        .unwrap();
    collaborator.sync().await.unwrap();

    owner.sync().await.unwrap();
    assert::all_paths(&owner, &["/", "/team/", "/team/notes/", "/team/notes/note.md"]).await;
}

/// An edit made inside a share that the same device then declines stays off the server, even when
/// another device of that account moved the edited folder into the share first.
#[tokio::test]
async fn edit_then_decline_share_another_device_moved_into() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    let note = recipient.create_at_path("/notes/note.md").await.unwrap();
    recipient.sync().await.unwrap();
    let other_device = another_client(&recipient).await;
    other_device.sync().await.unwrap();

    move_by_path(&recipient, "/notes/", "/team/").await.unwrap();
    recipient.sync().await.unwrap();

    other_device
        .write_document(note.id, b"draft")
        .await
        .unwrap();
    other_device.reject_share(&team.id).await.unwrap();
    other_device.sync().await.unwrap();
    other_device.sync().await.unwrap();
    assert::all_paths(&other_device, &["/"]).await;
}

/// One device moves its link to a share into a folder while another device moves that folder into
/// the share; the folder move is undone, as it would have been refused had the device known.
#[tokio::test]
async fn link_moved_into_folder_that_another_device_moved_into_the_share() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    recipient.create_at_path("/notes/").await.unwrap();
    recipient.sync().await.unwrap();
    let other_device = another_client(&recipient).await;
    other_device.sync().await.unwrap();

    let link = recipient.get_by_path("/team").await.unwrap();
    let notes = recipient.get_by_path("/notes/").await.unwrap();
    recipient.move_file(&link.id, &notes.id).await.unwrap();
    recipient.sync().await.unwrap();

    move_by_path(&other_device, "/notes/", "/team/")
        .await
        .unwrap();
    other_device.sync().await.unwrap();
    recipient.sync().await.unwrap();
    owner.sync().await.unwrap();

    assert::all_paths(&owner, &["/", "/team/"]).await;
    for device in [&recipient, &other_device] {
        assert::all_paths(device, &["/", "/notes/", "/notes/team/"]).await;
    }
}

/// A file shared with several people lists them in the order they were added, on every device.
#[tokio::test]
async fn shares_keep_their_order_through_a_sync() {
    let owner = test_core_with_account().await;
    let mut names = vec![];
    for _ in 0..5 {
        let recipient = test_core_with_account().await;
        names.push(recipient.get_account().unwrap().username.clone());
    }
    let note = owner.create_at_path("/note.md").await.unwrap();
    owner.sync().await.unwrap();

    for name in &names {
        owner
            .share_file(note.id, name, ShareMode::Read)
            .await
            .unwrap();
    }
    owner.sync().await.unwrap();

    let other_device = another_client(&owner).await;
    other_device.sync().await.unwrap();
    for device in [&owner, &other_device] {
        let shares = device.get_file_by_id(note.id).await.unwrap().shares;
        let order: Vec<_> = shares.into_iter().map(|s| s.shared_with).collect();
        assert_eq!(order, names);
    }
}

/// A recipient with links to a shared folder and to a folder inside it, whose own share of the
/// inner folder the owner has removed, declines the outer share and syncs an edit.
#[tokio::test]
async fn decline_share_holding_a_link_inside_it() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let trip = owner.create_at_path("/trip/").await.unwrap();
    let flights = owner.create_at_path("/trip/flights/").await.unwrap();
    owner
        .share_file(trip.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner
        .share_file(flights.id, &recipient_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();

    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/trip", trip.id)
        .await
        .unwrap();
    recipient
        .create_link_at_path("/flights", flights.id)
        .await
        .unwrap();
    let note = recipient.create_at_path("/note.md").await.unwrap();
    recipient.sync().await.unwrap();

    owner
        .unshare_file(flights.id, &recipient_name)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();

    recipient.reject_share(&trip.id).await.unwrap();
    recipient.write_document(note.id, b"packed").await.unwrap();
    recipient.sync().await.unwrap();
    assert::all_paths(&recipient, &["/", "/note.md"]).await;
}

/// Declining a share right after moving a folder into it puts the folder back where it was synced,
/// as the next sync would.
#[tokio::test]
async fn decline_share_after_moving_folder_into_it() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    recipient.create_at_path("/notes/note.md").await.unwrap();
    recipient.sync().await.unwrap();

    move_by_path(&recipient, "/notes/", "/team/").await.unwrap();
    recipient.reject_share(&team.id).await.unwrap();
    assert::all_paths(&recipient, &["/", "/notes/", "/notes/note.md"]).await;

    recipient.sync().await.unwrap();
    assert::all_paths(&recipient, &["/", "/notes/", "/notes/note.md"]).await;
}

/// A folder put back by declining a share takes the next free name when its old one was taken.
#[tokio::test]
async fn decline_share_after_moving_folder_into_it_and_reusing_its_name() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    recipient.create_at_path("/notes/note.md").await.unwrap();
    recipient.sync().await.unwrap();

    move_by_path(&recipient, "/notes/", "/team/").await.unwrap();
    recipient.create_at_path("/notes").await.unwrap();
    recipient.reject_share(&team.id).await.unwrap();
    let paths = ["/", "/notes", "/notes 2/", "/notes 2/note.md"];
    assert::all_paths(&recipient, &paths).await;

    recipient.sync().await.unwrap();
    assert::all_paths(&recipient, &paths).await;
}

/// Declining a share puts back each folder moved into it, and keeps a move into a folder that goes
/// back. Files are visited in no fixed order, so each case runs a few times.
#[tokio::test]
async fn decline_share_after_moving_nested_folders_into_it() {
    // each case: what's synced, the moves before declining, and what the device then shows
    type Case = (&'static str, &'static [(&'static str, &'static str)], &'static [&'static str]);
    let cases: [Case; 3] = [
        ("/a/b/", &[("/a/b/", "/team/"), ("/a/", "/team/")], &["/", "/a/", "/a/b/"]),
        ("/a/b/", &[("/a/b/", "/team/"), ("/a/", "/team/b/")], &["/", "/a/", "/a/b/"]),
        ("/b/", &[("/a/", "/team/"), ("/b/", "/team/a/")], &["/", "/a/", "/a/b/"]),
    ];
    for (synced, moves, paths) in cases {
        for _ in 0..8 {
            let owner = test_core_with_account().await;
            let recipient = test_core_with_account().await;
            let recipient_name = recipient.get_account().unwrap().username.clone();

            let team = owner.create_at_path("/team/").await.unwrap();
            owner
                .share_file(team.id, &recipient_name, ShareMode::Write)
                .await
                .unwrap();
            owner.sync().await.unwrap();
            recipient.sync().await.unwrap();
            recipient
                .create_link_at_path("/team", team.id)
                .await
                .unwrap();
            recipient.create_at_path(synced).await.unwrap();
            recipient.create_at_path("/a/").await.ok();
            recipient.sync().await.unwrap();

            for (from, to) in moves {
                move_by_path(&recipient, from, to).await.unwrap();
            }
            recipient.reject_share(&team.id).await.unwrap();
            assert::all_paths(&recipient, paths).await;

            recipient.sync().await.unwrap();
            assert::all_paths(&recipient, paths).await;
        }
    }
}

/// Declining a share still works after moving a folder into it out of a folder since deleted.
#[tokio::test]
async fn decline_share_after_moving_folder_into_it_and_deleting_its_old_parent() {
    let owner = test_core_with_account().await;
    let recipient = test_core_with_account().await;
    let recipient_name = recipient.get_account().unwrap().username.clone();

    let team = owner.create_at_path("/team/").await.unwrap();
    owner
        .share_file(team.id, &recipient_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    recipient.sync().await.unwrap();
    recipient
        .create_link_at_path("/team", team.id)
        .await
        .unwrap();
    let a = recipient.create_at_path("/a/b/").await.unwrap();
    recipient.sync().await.unwrap();

    move_by_path(&recipient, "/a/b/", "/team/").await.unwrap();
    recipient.delete(&a.parent).await.unwrap();
    recipient.reject_share(&team.id).await.unwrap();
    recipient.sync().await.unwrap();
}
