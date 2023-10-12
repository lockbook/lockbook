use lb_rs::Core;
use lockbook_shared::file::ShareMode;
use test_utils::*;

/// Tests that setup one device each on two accounts, share a file from one to the other, then sync both

fn assert_stuff(c1: &Core, c2: &Core) {
    for c in [c1, c2] {
        c.validate().unwrap();
        assert::local_work_paths(c, &[]);
        assert::server_work_paths(c, &[]);
        assert::deleted_files_pruned(c);
    }
    assert::all_paths(c2, &["/"]);
    assert::all_document_contents(c2, &[]);
}

#[test]
fn new_file() {
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

    assert_stuff(&cores[0], &cores[1]);
    assert::all_pending_shares(&cores[1], &["folder"]);
}

#[test]
fn new_files() {
    let cores = vec![test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let a = cores[0].create_at_path("a/").unwrap();
    cores[0].create_at_path("a/b/c/d").unwrap();
    let e = cores[0].create_at_path("e/").unwrap();
    cores[0].create_at_path("e/f/g/h").unwrap();
    cores[0]
        .share_file(a.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0]
        .share_file(e.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();

    assert_stuff(&cores[0], &cores[1]);
    assert::all_pending_shares(&cores[1], &["a", "e"]);
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

    assert_stuff(&cores[0], &cores[1]);
    assert::all_pending_shares(&cores[1], &["document"]);
}
