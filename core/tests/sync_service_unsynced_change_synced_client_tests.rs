use test_utils::*;

/// Tests that operate on one device after syncing.

#[test]
fn new_file() {
    let core = test_core_with_account();
    core.sync(None).unwrap();
    core.create_at_path("/document").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"")]);
    assert_local_work_paths(&core, &["/document"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn new_files() {
    let core = test_core_with_account();
    core.sync(None).unwrap();
    core.create_at_path("/a/b/c/d").unwrap();
    assert_all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert_all_document_contents(&core, &[("/a/b/c/d", b"")]);
    assert_local_work_paths(&core, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn edited_document() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    write_path(&core, "/document", b"document content").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"document content")]);
    assert_local_work_paths(&core, &["/document"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn edit_unedit() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    write_path(&core, "/document", b"document content").unwrap();
    write_path(&core, "/document", b"").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"")]);
    assert_local_work_paths(&core, &[]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn mv() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    let folder = core.create_at_path("/folder/").unwrap();
    core.sync(None).unwrap();
    core.move_file(doc.id, folder.id).unwrap();
    assert_all_paths(&core, &["/", "/folder/", "/folder/document"]);
    assert_all_document_contents(&core, &[("/folder/document", b"")]);
    assert_local_work_paths(&core, &["/folder/document"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn move_unmove() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    let folder = core.create_at_path("/folder/").unwrap();
    core.sync(None).unwrap();
    core.move_file(doc.id, folder.id).unwrap();
    core.move_file(doc.id, core.get_root().unwrap().id).unwrap();
    assert_all_paths(&core, &["/", "/folder/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"")]);
    assert_local_work_paths(&core, &[]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn rename() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    core.rename_file(doc.id, "document2").unwrap();
    assert_all_paths(&core, &["/", "/document2"]);
    assert_all_document_contents(&core, &[("/document2", b"")]);
    assert_local_work_paths(&core, &["/document2"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn rename_unrename() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    core.rename_file(doc.id, "document2").unwrap();
    core.rename_file(doc.id, "document").unwrap();
    assert_all_paths(&core, &["/", "/document"]);
    assert_all_document_contents(&core, &[("/document", b"")]);
    assert_local_work_paths(&core, &[]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn delete() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    delete_path(&core, "/document").unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &["/document"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn delete_parent() {
    let core = test_core_with_account();
    core.create_at_path("/parent/document").unwrap();
    core.sync(None).unwrap();
    delete_path(&core, "/parent/").unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &["/parent/"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}

#[test]
fn delete_grandparent() {
    let core = test_core_with_account();
    core.create_at_path("/grandparent/parent/document").unwrap();
    core.sync(None).unwrap();
    delete_path(&core, "/grandparent/").unwrap();
    assert_all_paths(&core, &["/"]);
    assert_all_document_contents(&core, &[]);
    assert_local_work_paths(&core, &["/grandparent/"]);
    core.validate().unwrap();
    assert_server_work_paths(&core, &[]);
}
