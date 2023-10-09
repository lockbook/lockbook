use lb_rs::Core;
use test_utils::*;

/// Tests that setup two synced clients, operate on one client, and sync it without syncing the
/// other client.

fn assert_stuff(c: &Core) {
    c.validate().unwrap();
    assert::local_work_paths(c, &[]);
}

#[test]
fn unmodified() {
    let c1 = test_core_with_account();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert::server_work_paths(&c2, &[]);
    assert_stuff(&c2);
}

#[test]
fn new_file() {
    let c1 = test_core_with_account();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert::server_work_paths(&c2, &["/document"]);
    assert_stuff(&c2);
}

#[test]
fn new_files() {
    let c1 = test_core_with_account();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/a/b/c/d").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
    assert::server_work_paths(&c2, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert_stuff(&c2);
}

#[test]
fn edited_document() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document", b"document content").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
    assert::server_work_paths(&c2, &["/document"]);
    assert_stuff(&c2);
}

#[test]
fn mv() {
    let c1 = test_core_with_account();
    let folder = c1.create_at_path("/folder/").unwrap();
    let doc = c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.move_file(doc.id, folder.id).unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/", "/folder/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
    assert::server_work_paths(&c2, &["/folder/document"]);
    assert_stuff(&c2);
}

#[test]
fn rename() {
    let c1 = test_core_with_account();
    let doc = c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.rename_file(doc.id, "document2").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
    assert::server_work_paths(&c2, &["/document2"]);
    assert_stuff(&c2);
}

#[test]
fn delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/document").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
    assert::server_work_paths(&c2, &["/document"]);
    assert_stuff(&c2);
}

#[test]
fn delete_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    c1.sync(None).unwrap();
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document"]);
    assert::all_document_contents(&c2, &[("/parent/document", b"")]);
    assert::server_work_paths(&c2, &["/parent/"]);
    assert_stuff(&c2);
}

#[test]
fn delete_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    c1.sync(None).unwrap();

    assert::all_paths(
        &c2,
        &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
    );
    assert::all_document_contents(&c2, &[("/grandparent/parent/document", b"")]);
    assert::server_work_paths(&c2, &["/grandparent/"]);
    assert_stuff(&c2);
}
