use lockbook_core::{Core, CoreError};
use lockbook_shared::file::ShareMode;
use test_utils::*;

/// Tests that setup one device each on two accounts, share a file from one to the other, sync both, then accept

fn assert_stuff(c1: &Core, c2: &Core) {
    for c in [c1, c2] {
        c.validate().unwrap();
        assert::local_work_paths(c, &[]);
        assert::server_work_paths(c, &[]);
        assert::deleted_files_pruned(c);
    }
    assert::all_pending_shares(c2, &[]);
}

#[test]
fn new_file() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[1], &["/", "/link"]);
}

#[test]
fn new_files() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").unwrap();
    cores[0].create_at_path("a/x/x").unwrap();
    let e = cores[0].create_at_path("e/").unwrap();
    cores[0].create_at_path("e/x/x").unwrap();
    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link1", shares[0].id).unwrap();
    cores[1].create_link_at_path("link2", shares[1].id).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    );
}

#[test]
fn move_file_a() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();

    let document = cores[0].create_at_path("document").unwrap();
    cores[0].sync(None).unwrap();
    cores[0].move_file(document.id, folder.id).unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
}

#[test]
fn create_file_in_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();
    let _document = cores[1].create_at_path("/link/document").unwrap();
    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
}

#[test]
fn move_file_b() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();
    let document = cores[1].create_at_path("document").unwrap();
    cores[1].move_file(document.id, folder.id).unwrap();
    cores[1].sync(None).unwrap();

    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]);
    assert::all_paths(&cores[1], &["/", "/link/", "/link/document"]);
}

#[test]
fn move_file_with_child() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();
    let folder2 = cores[1].create_at_path("folder2/").unwrap();
    let document = cores[1].create_at_path("folder2/document").unwrap();
    cores[1]
        .write_document(document.id, b"document content")
        .unwrap();
    cores[1].move_file(folder2.id, folder.id).unwrap();
    cores[1].sync(None).unwrap();

    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(
        &cores[0],
        &["/", "/folder/", "/folder/folder2/", "/folder/folder2/document"],
    );
    assert::all_paths(&cores[1], &["/", "/link/", "/link/folder2/", "/link/folder2/document"]);
    assert::all_document_contents(&cores[0], &[("/folder/folder2/document", b"document content")]);
    assert::all_document_contents(&cores[1], &[("/link/folder2/document", b"document content")]);
}

#[test]
fn delete_accepted_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].delete_pending_share(folder.id).unwrap();
    cores[1].sync(None).unwrap(); // this succeeds...
    cores[1].sync(None).unwrap(); // ...and this fails (before being fixed)
    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[1], &["/"]);
}

#[test]
fn synced_files() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").unwrap();
    let axx = cores[0].create_at_path("a/x/x").unwrap();
    let e = cores[0].create_at_path("e/").unwrap();
    cores[0].create_at_path("e/x/x").unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link1", shares[0].id).unwrap();
    cores[1].create_link_at_path("link2", shares[1].id).unwrap();

    cores[0]
        .write_document(axx.id, b"document content")
        .unwrap();
    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    );
}

#[test]
fn synced_files_edit_after_share() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").unwrap();
    let axx = cores[0].create_at_path("a/x/x").unwrap();
    let e = cores[0].create_at_path("e/").unwrap();
    cores[0].create_at_path("e/x/x").unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link1", shares[0].id).unwrap();
    cores[1].create_link_at_path("link2", shares[1].id).unwrap();

    cores[0]
        .write_document(axx.id, b"document content")
        .unwrap();
    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(
        &cores[1],
        &["/", "/link1/", "/link1/x/", "/link1/x/x", "/link2/", "/link2/x/", "/link2/x/x"],
    );
}

#[test]
fn edited_document() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("document").unwrap();
    cores[0]
        .write_document(document.id, b"document content")
        .unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].create_link_at_path("link", shares[0].id).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[1], &["/", "/link"]);
    assert::all_document_contents(&cores[1], &[("/linkdocument", b"document content")]);
}

#[test]
fn move_existing_edited_document_into_shared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let shares = cores[1].get_pending_shares().unwrap();
    let _link = cores[1].create_link_at_path("link/", shares[0].id).unwrap();
    let document = cores[1].create_at_path("document").unwrap();

    cores[1].sync(None).unwrap();

    cores[1].move_file(document.id, folder.id).unwrap();
    cores[1]
        .write_document(document.id, b"document content")
        .unwrap();

    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/document"]);
    assert::all_document_contents(&cores[0], &[("/folder/document", b"document content")]);
}

#[test]
fn create_link_in_unshared_folder() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let shares = cores[1].get_pending_shares().unwrap();
    cores[1].delete_pending_share(shares[0].id).unwrap();
    let document = cores[1].create_at_path("document").unwrap();
    cores[1]
        .share_file(document.id, &accounts[0].username, ShareMode::Write)
        .unwrap();

    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();

    let shares = cores[0].get_pending_shares().unwrap();
    let _link = cores[0]
        .create_link_at_path("folder/link", shares[0].id)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[0], &["/", "/folder/", "/folder/link"]);
    assert::all_paths(&cores[1], &["/", "/document"]); // NOTE: fails here; document has vanished because it's path s resolved using a shared link
}

#[test]
fn move_file_out_of_shared_folder_and_delete() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    let document = cores[0].create_at_path("folder/document").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let shares = cores[1].get_pending_shares().unwrap();
    let _link = cores[1].create_link_at_path("link/", shares[0].id).unwrap();

    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();

    cores[0].move_file(document.id, roots[0].id).unwrap();

    cores[0].sync(None).unwrap();

    cores[0].delete_file(document.id).unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_paths(&cores[1], &["/", "/link/"]); // originally, failed here; deletion wasn't synced because file was no longer in user's tree
}

#[test]
fn move_file_out_of_shared_folder_and_create_path_conflict() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();
    let roots = cores
        .iter()
        .map(|core| core.get_root().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0].create_at_path("folder/").unwrap();
    let document = cores[0].create_at_path("folder/document").unwrap();
    cores[0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    let shares = cores[1].get_pending_shares().unwrap();
    let _link = cores[1].create_link_at_path("link/", shares[0].id).unwrap();

    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();

    cores[0].move_file(document.id, roots[0].id).unwrap();

    cores[0].sync(None).unwrap();

    cores[0].delete_file(document.id).unwrap();
    cores[0].create_at_path("/folder/document").unwrap(); // originally, this would conflict with the now-moved document whose move wasn't synced to the other client

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    // this call now returns an error, but the code is preserved for the original reproduction of the bug
    if cores[1].get_file_by_id(document.id).is_ok() {
        cores[1]
            .write_document(document.id, b"document content")
            .unwrap(); // originally, this would fail with Unexpected("PathTaken")

        cores[1].sync(None).unwrap();

        assert_stuff(&cores[0], &cores[1]); // originally, if the test did make it here, validation would fail with a path conflict
    }
}

#[test]
fn test_share_link_write() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[2].sync(None).unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]);
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]);
    assert_matches!(
        cores[1]
            .share_file(link.id, &accounts[2].username, ShareMode::Write)
            .unwrap_err()
            .kind, // this succeeded and now correctly fails (was sharing link instead of target)
        CoreError::InsufficientPermission
    );
    cores[1].sync(None).unwrap();

    cores[2].sync(None).unwrap(); // this failed with FileNonexistent
}

#[test]
fn test_share_link_read() {
    let cores = vec![test_core_with_account(), test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();
    cores[2].sync(None).unwrap();

    let passalong = cores[0].create_at_path("/passalong.md").unwrap();
    cores[0]
        .share_file(passalong.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    assert::all_pending_shares(&cores[1], &["passalong.md"]);
    let link = cores[1]
        .create_link_at_path("/passalong.md", passalong.id)
        .unwrap();
    assert::all_paths(&cores[1], &["/", "/passalong.md"]);
    cores[1]
        .share_file(link.id, &accounts[2].username, ShareMode::Read)
        .unwrap();
    cores[1].sync(None).unwrap();

    cores[2].sync(None).unwrap();
}
