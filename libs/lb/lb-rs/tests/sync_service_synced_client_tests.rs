use lb_rs::Core;
use test_utils::*;

/// Tests that operate on one device and sync (work should be none, deleted files should be pruned)

fn assert_stuff(core: &Core) {
    core.validate().unwrap();
    assert::local_work_paths(core, &[]);
    assert::server_work_paths(core, &[]);
    assert::deleted_files_pruned(core);
    assert::new_synced_client_core_equal(core);
}

#[test]
fn unmodified() {
    let core = test_core_with_account();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/"]);
    assert::all_document_contents(&core, &[]);
    assert_stuff(&core);
}

#[test]
fn new_file() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/", "/document"]);
    assert::all_document_contents(&core, &[("/document", b"")]);
    assert_stuff(&core);
}

#[test]
fn new_file_name_same_as_username() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    core.create_at_path(&format!("/{}", &account.username))
        .unwrap();
    core.sync(None).unwrap();
    let account = core.get_account().unwrap();
    let document_path = format!("/{}", account.username);
    assert::all_paths(&core, &["/", &document_path]);
    assert::all_document_contents(&core, &[(&document_path, b"")]);
    assert_stuff(&core);
}

#[test]
fn new_files() {
    let core = test_core_with_account();
    core.create_at_path("/a/b/c/d").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
    assert::all_document_contents(&core, &[("/a/b/c/d", b"")]);
    assert_stuff(&core);
}

#[test]
fn edited_document() {
    let core = test_core_with_account();
    core.create_at_path("/document").unwrap();
    write_path(&core, "/document", b"document content").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/", "/document"]);
    assert::all_document_contents(&core, &[("/document", b"document content")]);
    assert_stuff(&core);
}

#[test]
fn mv() {
    let core = test_core_with_account();
    let folder = core.create_at_path("/folder/").unwrap();
    let doc = core.create_at_path("/document").unwrap();
    core.move_file(doc.id, folder.id).unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/", "/folder/", "/folder/document"]);
    assert::all_document_contents(&core, &[("/folder/document", b"")]);
    assert_stuff(&core);
}

#[test]
fn rename() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.rename_file(doc.id, "document2").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/", "/document2"]);
    assert::all_document_contents(&core, &[("/document2", b"")]);
    assert_stuff(&core);
}

#[test]
fn delete() {
    let core = test_core_with_account();
    let doc = core.create_at_path("/document").unwrap();
    core.delete_file(doc.id).unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/"]);
    assert::all_document_contents(&core, &[]);
    assert_stuff(&core);
}

#[test]
fn delete_parent() {
    let core = test_core_with_account();
    core.create_at_path("/folder/document").unwrap();
    delete_path(&core, "/folder/").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/"]);
    assert::all_document_contents(&core, &[]);
    assert_stuff(&core);
}

#[test]
fn delete_grandparent() {
    let core = test_core_with_account();
    core.create_at_path("/grandparent/parent/document").unwrap();
    delete_path(&core, "/grandparent/").unwrap();
    core.sync(None).unwrap();
    assert::all_paths(&core, &["/"]);
    assert::all_document_contents(&core, &[]);
    assert_stuff(&core);
}
