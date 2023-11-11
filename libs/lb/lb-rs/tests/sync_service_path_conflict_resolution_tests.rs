use lb_rs::Core;
use test_utils::*;

/// Tests which are constructed to test path conflict resolution Like those above, these are tests
/// that setup two synced clients, operate on both clients, then sync both twice (work should be
/// none, client dbs should be equal, deleted files should be pruned).

fn sync_and_assert_stuff(c1: &Core, c2: &Core) {
    c1.sync(None).unwrap();
    c2.sync(None).unwrap();
    c1.sync(None).unwrap();
    c2.sync(None).unwrap();

    c1.validate().unwrap();
    assert::cores_equal(c1, c2);
    assert::local_work_paths(c1, &[]);
    assert::server_work_paths(c1, &[]);
    assert::deleted_files_pruned(c1);
}

#[test]
fn concurrent_create_documents() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a.md").unwrap();
    c2.create_at_path("/a.md").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md"]);
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
}

#[test]
fn concurrent_create_folders() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a/").unwrap();
    c2.create_at_path("/a/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a/", "/a-1/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn concurrent_create_folders_with_children() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a/child/").unwrap();
    c2.create_at_path("/a/child/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a/", "/a-1/", "/a/child/", "/a-1/child/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn concurrent_create_document_then_folder() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a.md").unwrap();
    c2.create_at_path("/a.md/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md/"]);
    assert::all_document_contents(&c2, &[("/a.md", b"")]);
}

#[test]
fn concurrent_create_folder_then_document() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a.md/").unwrap();
    c2.create_at_path("/a.md").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a.md/", "/a-1.md"]);
    assert::all_document_contents(&c2, &[("/a-1.md", b"")]);
}

#[test]
fn concurrent_create_document_then_folder_with_child() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a.md").unwrap();
    c2.create_at_path("/a.md/child/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a.md", "/a-1.md/", "/a-1.md/child/"]);
    assert::all_document_contents(&c2, &[("/a.md", b"")]);
}

#[test]
fn concurrent_create_folder_with_child_then_document() {
    let c1 = test_core_with_account();
    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    c1.create_at_path("/a.md/child/").unwrap();
    c2.create_at_path("/a.md").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/a.md/", "/a.md/child/", "/a-1.md"]);
    assert::all_document_contents(&c2, &[("/a-1.md", b"")]);
}

#[test]
fn concurrent_move_then_create_documents() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/folder/a.md", "").unwrap();
    c2.create_at_path("/a.md").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]);
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
}

#[test]
fn concurrent_create_then_move_documents() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/a.md").unwrap();
    move_by_path(&c2, "/folder/a.md", "").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]);
    assert::all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
}

#[test]
fn concurrent_move_then_create_folders() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/folder/a.md/", "").unwrap();
    c2.create_at_path("/a.md/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn concurrent_create_then_move_folders() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/a.md/").unwrap();
    move_by_path(&c2, "/folder/a.md/", "").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn concurrent_move_then_create_folders_with_children() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md/child/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/folder/a.md/", "").unwrap();
    c2.create_at_path("/a.md/child/").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(
        &c2,
        &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
    );
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn concurrent_create_then_move_folders_with_children() {
    let c1 = test_core_with_account();
    c1.create_at_path("/folder/a.md/child/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/a.md/child/").unwrap();
    move_by_path(&c2, "/folder/a.md/", "").unwrap();

    sync_and_assert_stuff(&c1, &c2);
    assert::all_paths(
        &c2,
        &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
    );
    assert::all_document_contents(&c2, &[]);
}
