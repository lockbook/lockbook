use lockbook_core::Core;
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
fn move_file() {
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
    assert::all_document_contents(&cores[1], &[("/link", b"document content")]);
}
