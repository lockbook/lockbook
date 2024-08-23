use lb_rs::logic::file::ShareMode;
use lb_rs::logic::file_metadata::FileType;
use lb_rs::Core;
use test_utils::*;

/// Tests that setup two synced devices, operate on both devices, then sync both twice (work
/// should be none, devices dbs should be equal, deleted files should be pruned).

fn sync_and_assert(c1: &Core, c2: &Core) {
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
fn identical_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document", "/parent/").unwrap();
    move_by_path(&c2, "/document", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document"]);
    assert::all_document_contents(&c2, &[("/parent/document", b"")]);
}

#[test]
fn different_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/parent2/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document", "/parent/").unwrap();
    move_by_path(&c2, "/document", "/parent2/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent2/", "/parent/document"]);
    assert::all_document_contents(&c2, &[("/parent/document", b"")]);
}

#[test]
fn identical_rename() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/document", "document2").unwrap();
    rename_path(&c2, "/document", "document2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document2"]);
    assert::all_document_contents(&c2, &[("/document2", b"")]);
}

#[test]
fn different_rename() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/document", "document2").unwrap();
    rename_path(&c2, "/document", "document3").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document2"]);
    assert::all_document_contents(&c2, &[("/document2", b"")]);
}

#[test]
fn move_then_rename() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document", "/parent/").unwrap();
    rename_path(&c2, "/document", "document2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document2"]);
    assert::all_document_contents(&c2, &[("/parent/document2", b"")]);
}

#[test]
fn rename_then_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/document", "document2").unwrap();
    move_by_path(&c2, "/document", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document2"]);
    assert::all_document_contents(&c2, &[("/parent/document2", b"")]);
}

#[test]
fn identical_delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/document").unwrap();
    delete_path(&c2, "/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn identical_delete_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_parent_then_direct() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    delete_path(&c2, "/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_direct_then_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/document").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn identical_delete_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    delete_path(&c2, "/grandparent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_grandparent_then_direct() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    delete_path(&c2, "/grandparent/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_direct_then_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/parent/document").unwrap();
    delete_path(&c2, "/grandparent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_grandparent_then_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    delete_path(&c2, "/grandparent/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_parent_then_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/parent/").unwrap();
    delete_path(&c2, "/grandparent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn move_then_delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document", "/parent/").unwrap();
    delete_path(&c2, "/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_then_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/document").unwrap();
    move_by_path(&c2, "/document", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn move_then_delete_new_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document", "/parent/").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_new_parent_then_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    move_by_path(&c2, "/document", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn move_then_delete_old_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/parent/document", "").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document"]);
    assert::all_document_contents(&c2, &[("/document", b"")]);
}

#[test]
fn delete_old_parent_then_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    move_by_path(&c2, "/parent/document", "").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn rename_then_delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/document", "document2").unwrap();
    delete_path(&c2, "/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_then_rename() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/document").unwrap();
    rename_path(&c2, "/document", "document2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn create_then_move_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/parent2/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/parent/document").unwrap();
    move_by_path(&c2, "/parent/", "/parent2/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"]);
    assert::all_document_contents(&c2, &[("/parent2/parent/document", b"")]);
}

#[test]
fn move_parent_then_create() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/parent2/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/parent/", "/parent2/").unwrap();
    c2.create_at_path("/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"]);
    assert::all_document_contents(&c2, &[("/parent2/parent/document", b"")]);
}

#[test]
fn create_then_rename_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/parent/document").unwrap();
    rename_path(&c2, "/parent/", "parent2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/document"]);
    assert::all_document_contents(&c2, &[("/parent2/document", b"")]);
}

#[test]
fn rename_parent_then_create() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/parent/", "parent2").unwrap();
    c2.create_at_path("/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent2/", "/parent2/document"]);
    assert::all_document_contents(&c2, &[("/parent2/document", b"")]);
}

#[test]
fn create_then_delete_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/parent/document").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_parent_then_create() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    c2.create_at_path("/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn create_then_delete_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    c1.create_at_path("/grandparent/parent/document").unwrap();
    delete_path(&c2, "/grandparent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_grandparent_then_create() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    c2.create_at_path("/grandparent/parent/document").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn identical_content_edit_not_mergable() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.draw").unwrap();
    write_path(&c1, "/document.draw", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.draw", b"document content 2").unwrap();
    write_path(&c2, "/document.draw", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document.draw"]);
    assert::all_document_contents(&c2, &[("/document.draw", b"document content 2")]);
}

#[test]
fn identical_content_edit_mergable() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document content 2").unwrap();
    write_path(&c2, "/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document.md"]);
    assert::all_document_contents(&c2, &[("/document.md", b"document content 2")]);
}

#[test]
fn different_content_edit_not_mergable() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.draw").unwrap();
    write_path(&c1, "/document.draw", b"document\n\ncontent\n").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.draw", b"document 2\n\ncontent\n").unwrap();
    write_path(&c2, "/document.draw", b"document\n\ncontent 2\n").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document.draw", "/document-1.draw"]);
    assert::all_document_contents(
        &c2,
        &[
            ("/document.draw", b"document 2\n\ncontent\n"),
            ("/document-1.draw", b"document\n\ncontent 2\n"),
        ],
    );
}

#[test]
fn different_content_edit_mergable() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n").unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document.md"]);
    assert::all_document_contents(&c2, &[("/document.md", b"document 2\n\ncontent 2\n")]);
}

#[test]
fn different_content_edit_mergable_with_move_in_first_sync() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n").unwrap();
    move_by_path(&c1, "/document.md", "/parent/").unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]);
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document 2\n\ncontent 2\n")]);
}

#[test]
fn different_content_edit_mergable_with_move_in_second_sync() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document\n\ncontent\n").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document 2\n\ncontent\n").unwrap();
    write_path(&c2, "/document.md", b"document\n\ncontent 2\n").unwrap();
    move_by_path(&c2, "/document.md", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]);
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document 2\n\ncontent 2\n")]);
}

#[test]
fn move_then_edit_content() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    move_by_path(&c1, "/document.md", "/parent/").unwrap();
    write_path(&c2, "/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]);
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document content 2")]);
}

#[test]
fn edit_content_then_move() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/").unwrap();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document content 2").unwrap();
    move_by_path(&c2, "/document.md", "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/parent/", "/parent/document.md"]);
    assert::all_document_contents(&c2, &[("/parent/document.md", b"document content 2")]);
}

#[test]
fn rename_then_edit_content() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    rename_path(&c1, "/document.md", "document2.md").unwrap();
    write_path(&c2, "/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document2.md"]);
    assert::all_document_contents(&c2, &[("/document2.md", b"document content 2")]);
}

#[test]
fn edit_content_then_rename() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document content 2").unwrap();
    rename_path(&c2, "/document.md", "document2.md").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/", "/document2.md"]);
    assert::all_document_contents(&c2, &[("/document2.md", b"document content 2")]);
}

#[test]
fn delete_then_edit_content() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/document.md").unwrap();
    write_path(&c2, "/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn edit_content_then_delete() {
    let c1 = test_core_with_account();
    c1.create_at_path("/document.md").unwrap();
    write_path(&c1, "/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/document.md", b"document content 2").unwrap();
    delete_path(&c2, "/document.md").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_parent_then_edit_content() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document.md").unwrap();
    write_path(&c1, "/parent/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/parent/").unwrap();
    write_path(&c2, "/parent/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn edit_content_then_delete_parent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/parent/document.md").unwrap();
    write_path(&c1, "/parent/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/parent/document.md", b"document content 2").unwrap();
    delete_path(&c2, "/parent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn delete_grandparent_then_edit_content() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document.md")
        .unwrap();
    write_path(&c1, "/grandparent/parent/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    delete_path(&c1, "/grandparent/").unwrap();
    write_path(&c2, "/grandparent/parent/document.md", b"document content 2").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn edit_content_then_delete_grandparent() {
    let c1 = test_core_with_account();
    c1.create_at_path("/grandparent/parent/document.md")
        .unwrap();
    write_path(&c1, "/grandparent/parent/document.md", b"document content").unwrap();
    c1.sync(None).unwrap();

    let c2 = another_client(&c1);
    c2.sync(None).unwrap();

    write_path(&c1, "/grandparent/parent/document.md", b"document content 2").unwrap();
    delete_path(&c2, "/grandparent/").unwrap();

    sync_and_assert(&c1, &c2);
    assert::all_paths(&c2, &["/"]);
    assert::all_document_contents(&c2, &[]);
}

#[test]
fn create_two_links() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .create_link_at_path("/link1", document.id)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .create_link_at_path("/link2", document.id)
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);
    assert::all_paths(&cores[1][0], &["/", "/link1"]);
}

#[test]
fn share_then_create_link_in_folder() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    let folder = cores[1][0].create_at_path("/folder/").unwrap();
    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .share_file(folder.id, &accounts[0].username, ShareMode::Read)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .create_link_at_path("/folder/link", document.id)
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);
    assert::all_paths(&cores[1][0], &["/", "/folder/"]);
}

#[test]
fn create_link_in_folder_then_share() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    let folder = cores[1][0].create_at_path("/folder/").unwrap();
    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .share_file(folder.id, &accounts[0].username, ShareMode::Read)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .create_link_at_path("/folder/link", document.id)
        .unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]); // note: reverse order from above test
    assert::all_paths(&cores[1][0], &["/", "/folder/", "/folder/link"]);
}

#[test]
fn create_link_then_move_to_owned_folder() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").unwrap();
    let document = cores[0][0].create_at_path("/folder/document").unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .move_file(document.id, cores[1][1].get_root().unwrap().id)
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);

    assert::all_paths(&cores[1][0], &["/", "/link"]);
}

#[test]
fn create_link_then_move_to_owned_folder_and_move_prior_parent_into_it() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let grandparent = cores[0][0].create_at_path("/grandparent/").unwrap();
    let parent = cores[0][0].create_at_path("/grandparent/parent/").unwrap();
    let folder = cores[0][0]
        .create_at_path("/grandparent/parent/child/")
        .unwrap();
    cores[0][0]
        .share_file(grandparent.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0].create_link_at_path("/link", folder.id).unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .move_file(folder.id, cores[1][1].get_root().unwrap().id)
        .unwrap();
    cores[1][1].move_file(parent.id, folder.id).unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);

    assert::all_paths(&cores[1][0], &["/", "/link/"]);
}

#[test]
fn create_link_then_move_to_owned_folder_and_create_file_with_conflicting_name_in_prior_parent() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let grandparent = cores[0][0].create_at_path("/grandparent/").unwrap();
    let parent = cores[0][0].create_at_path("/grandparent/parent/").unwrap();
    let folder = cores[0][0]
        .create_at_path("/grandparent/parent/folder/")
        .unwrap();
    cores[0][0]
        .share_file(grandparent.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0].create_link_at_path("/link", folder.id).unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .move_file(folder.id, cores[1][1].get_root().unwrap().id)
        .unwrap();
    let _new_folder = cores[1][1]
        .create_file("folder", parent.id, FileType::Folder)
        .unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);

    assert::all_paths(&cores[1][0], &["/", "/link/"]);
    assert::all_paths(
        &cores[0][0],
        &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/folder/"],
    );
}

#[test]
fn move_to_owned_folder_then_create_link() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").unwrap();
    let document = cores[0][0].create_at_path("/folder/document").unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1]
        .move_file(document.id, cores[1][1].get_root().unwrap().id)
        .unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]); // note: reverse order from above test
    assert::all_paths(&cores[1][0], &["/", "/document"]);
}

#[test]
fn create_link_then_delete() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").unwrap();
    let document = cores[0][0].create_at_path("/folder/document").unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1].delete_file(document.id).unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);

    assert::all_paths(&cores[1][0], &["/"]);
}

#[test]
fn delete_then_create_link() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let folder = cores[0][0].create_at_path("/folder/").unwrap();
    let document = cores[0][0].create_at_path("/folder/document").unwrap();
    cores[0][0]
        .share_file(folder.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .create_link_at_path("/link", document.id)
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1].delete_file(document.id).unwrap();

    sync_and_assert(&cores[1][1], &cores[1][0]); // note: order reversed from above test

    assert::all_paths(&cores[1][0], &["/"]);
}

#[test]
fn share_from_two_clients() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[0][0]);
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0].sync(None).unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0][1].sync(None).unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0][0].sync(None).unwrap();
    cores[0][1].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    cores[1][0].sync(None).unwrap();
}

#[test]
fn share_from_two_clients_read_then_write() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[0][0]);
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0].sync(None).unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();

    cores[0][1].sync(None).unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0][0].sync(None).unwrap();
    cores[0][1].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    cores[1][0].sync(None).unwrap();
}

#[test]
fn share_from_two_clients_write_then_read() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[0][0]);
    cores[0].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0].sync(None).unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[0][1].sync(None).unwrap();
    cores[0][1]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();

    cores[0][0].sync(None).unwrap();
    cores[0][1].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    cores[1][0].sync(None).unwrap();
}

#[test]
fn share_delete_then_upgrade_to_write() {
    let cores = [test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1].delete_pending_share(document.id).unwrap();

    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    cores[1].sync(None).unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap();
    cores[1].sync(None).unwrap();
}

#[test]
fn share_upgrade_to_write_then_delete() {
    let cores = [test_core_with_account(), test_core_with_account()];
    let accounts = cores
        .iter()
        .map(|core| core.get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0].create_at_path("/document").unwrap();
    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Read)
        .unwrap();
    cores[0].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1].delete_pending_share(document.id).unwrap();

    cores[0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();

    // note: sync order reversed from above test
    cores[0].sync(None).unwrap();
    cores[1].sync(None).unwrap();

    cores[1].sync(None).unwrap();
    cores[1]
        .write_document(document.id, b"document content by sharee")
        .unwrap_err();
}

#[test]
fn deleted_share_of_file_with_local_change() {
    let mut cores = [vec![test_core_with_account()], vec![test_core_with_account()]];
    let new_client = another_client(&cores[1][0]);
    cores[1].push(new_client);
    let accounts = cores
        .iter()
        .map(|cores| cores[0].get_account().unwrap())
        .collect::<Vec<_>>();

    let document = cores[0][0].create_at_path("/document").unwrap();
    cores[0][0]
        .write_document(document.id, b"document content by sharer")
        .unwrap();
    cores[0][0]
        .share_file(document.id, &accounts[1].username, ShareMode::Write)
        .unwrap();
    cores[0][0].sync(None).unwrap();

    cores[1][0].sync(None).unwrap();
    cores[1][0]
        .write_document(document.id, b"document content by sharee")
        .unwrap();

    cores[1][1].sync(None).unwrap();
    cores[1][1].delete_pending_share(document.id).unwrap();

    sync_and_assert(&cores[1][0], &cores[1][1]);

    assert::all_paths(&cores[1][0], &["/"]);
}
