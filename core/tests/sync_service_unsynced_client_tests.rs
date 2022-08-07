use test_utils::*;

/// Tests that operate on one client without syncing.

#[test]
fn unmodified() {
    let core = test_core_with_account();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &[]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn new_file() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"")]);
    assert_local_work_paths(&core, &["/document"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn new_files() {
    let core = test_core_with_account();
    core.create_at_path("/a/b/c/d").unwrap();
    assert_all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert_all_document_contents(&core, &[("/a/b/c/d", b"")]);
    assert_local_work_paths(&core, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn edited_document() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    write_path(&core, "/document", b"document content").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"document content")]);
    assert_local_work_paths(&core, &["/document"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn mv() {
    let core = test_core_with_account();
    let new_parent = core.create_at_path("/folder/").unwrap();
    let doc = core.create_at_path("/document").unwrap();
    core.move_file(doc.id, new_parent.id).unwrap();
    assert_all_paths(&core, &["/", "/folder/", "/folder/document"]);
    assert_all_document_contents(&core, &[("/folder/document", b"")]);
    assert_local_work_paths(&core, &["/folder/", "/folder/document"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn rename() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.rename_file(doc.id, "document2").unwrap();
    assert_all_paths(&core, &["/", "/document2"]);
    assert_all_document_contents(&core, &[("/document2", b"")]);
    assert_local_work_paths(&core, &["/document2"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn delete() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.delete_file(doc.id).unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &["/document"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn delete_parent() {
    let core = test_core_with_account();
    core.create_at_path("/parent/document").unwrap();
    delete_path(&core, "/parent/").unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &["/parent/", "/parent/document"]);
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}

#[test]
fn delete_grandparent() {
    let core = test_core_with_account();
    core.create_at_path("/grandparent/parent/document").unwrap();
    delete_path(&core, "/grandparent/").unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(
        &core,
        &["/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
    );
    assert_server_work_paths(&core, &[]);
    core.validate().unwrap();
}
