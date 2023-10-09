use lb_rs::Core;
use test_utils::*;

/// Tests that operate on one client, sync it, then create and sync a new client (work should be
/// none, devices dbs should be equal, deleted files should be pruned).

fn assert_stuff(c1: &Core, c2: &Core) {
    c1.validate().unwrap();
    assert::cores_equal(c1, c2);
    assert::local_work_paths(c1, &[]);
    assert::server_work_paths(c1, &[]);
    assert::deleted_files_pruned(c1);
    assert::new_synced_client_core_equal(c1);
}

#[test]
fn unmodified() {
    let c1 = test_core_with_account();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert_stuff(&c1, &c2);
}

#[test]
fn new_file() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
    assert_stuff(&c1, &c2);
}

#[test]
fn new_files() {
    let c1 = test_core_with_account();
    c1.create_at_path("/a/b/c/d").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert::all_document_contents(&c2, &[("/a/b/c/d", b"")]);
    assert_stuff(&c1, &c2);
}

#[test]
fn edited_document() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    write_path(&c1, "/document", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"document content")]);
    assert_stuff(&c1, &c2);
}

#[test]
fn mv() {
    let c1 = test_core_with_account();
    let folder = c1.create_at_path("/folder/").unwrap();
    let doc = c1.create_at_path("/document").unwrap();
    c1.move_file(doc.id, folder.id).unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/folder/", "/folder/document"]);
    assert::all_document_contents(&c2, &[("/folder/document", b"")]);
    assert_stuff(&c1, &c2);
}

#[test]
fn rename() {
    let c1 = test_core_with_account();
    let doc = c1.create_at_path("/document").unwrap();
    c1.rename_file(doc.id, "document2").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/document2"]);
    assert::all_document_contents(&c2, &[("/document2", b"")]);
    assert_stuff(&c1, &c2);
}

#[test]
fn delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    delete_path(&c1, "/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert_stuff(&c1, &c2);
}

#[test]
fn delete_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    delete_path(&c1, "/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert_stuff(&c1, &c2);
}

#[test]
fn delete_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    delete_path(&c1, "/grandparent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert_stuff(&c1, &c2);
}
