use hmdb::transaction::Transaction;
use itertools::Itertools;
use lockbook_core::model::repo::RepoSource;
use lockbook_core::pure_functions::files;
use test_utils::Operation::*;
use test_utils::*;

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device without syncing
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn unsynced_device() {
    for mut ops in [
        // unmodified
        vec![Custom {
            f: &|dbs, root| {
                let db = &dbs[0];
                test_utils::assert_all_paths(db, root, &[""]);
                test_utils::assert_all_document_contents(db, root, &[]);
                test_utils::assert_local_work_paths(db, root, &[]);
                test_utils::assert_server_work_paths(db, root, &[]);
            },
        }],
        // new_file
        vec![
            Create { client_num: 0, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["document"]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // new_files
        vec![
            Create { client_num: 0, path: "a/b/c/d" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a/b/c/d", b"")]);
                    test_utils::assert_local_work_paths(
                        db,
                        root,
                        &["a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document", b"document content")],
                    );
                    test_utils::assert_local_work_paths(db, root, &["document"]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "folder/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("folder/document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["folder/", "folder/document"]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["document2"]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Delete { client_num: 0, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Delete { client_num: 0, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 0, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, _| {
                let db = &dbs[0];
                db.validate().unwrap();
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device and sync
    (work should be none, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn synced_device() {
    for mut ops in [
        // unmodified
        vec![
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // new_file
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                },
            },
        ],
        // new_file_name_same_as_username
        vec![
            Custom {
                f: &|dbs, _| {
                    let db = &dbs[0];
                    let account = db.get_account().unwrap();
                    db.create_at_path(&path(db, &account.username)).unwrap();
                },
            },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    let account = db.get_account().unwrap();
                    let document_path = account.username;
                    test_utils::assert_all_paths(db, root, &["", &document_path]);
                    test_utils::assert_all_document_contents(db, root, &[(&document_path, b"")]);
                },
            },
        ],
        // new_files
        vec![
            Create { client_num: 0, path: "a/b/c/d" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a/b/c/d", b"")]);
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document", b"document content")],
                    );
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "folder/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("folder/document", b"")]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Delete { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "folder/document" },
            Delete { client_num: 0, path: "folder/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 0, path: "grandparent/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[0];
                db.validate().unwrap();
                test_utils::assert_local_work_paths(db, root, &[]);
                test_utils::assert_server_work_paths(db, root, &[]);
                test_utils::assert_deleted_files_pruned(db);
                test_utils::assert_new_synced_client_dbs_eq(db);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device after syncing
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn unsynced_change_synced_device() {
    for mut ops in [
        // new_file
        vec![
            Sync { client_num: 0 },
            Create { client_num: 0, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["document"]);
                },
            },
        ],
        // new_files
        vec![
            Sync { client_num: 0 },
            Create { client_num: 0, path: "a/b/c/d" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a/b/c/d", b"")]);
                    test_utils::assert_local_work_paths(
                        db,
                        root,
                        &["a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document", b"document content")],
                    );
                    test_utils::assert_local_work_paths(db, root, &["document"]);
                },
            },
        ],
        // edit_unedit
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Edit { client_num: 0, path: "document", content: b"" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "document" },
            Create { client_num: 0, path: "folder/" },
            Sync { client_num: 0 },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "folder/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("folder/document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["folder/document"]);
                },
            },
        ],
        // move_unmove
        vec![
            Create { client_num: 0, path: "document" },
            Create { client_num: 0, path: "folder/" },
            Sync { client_num: 0 },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Move { client_num: 0, path: "folder/document", new_parent_path: "" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                    test_utils::assert_local_work_paths(db, root, &["document2"]);
                },
            },
        ],
        // rename_unrename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Rename { client_num: 0, path: "document2", new_name: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_local_work_paths(db, root, &[]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Delete { client_num: 0, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &["document"]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Delete { client_num: 0, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &["parent/"]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Delete { client_num: 0, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_local_work_paths(db, root, &["grandparent/"]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[0];
                db.validate().unwrap();
                test_utils::assert_server_work_paths(db, root, &[]);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device, sync it, then create a new device without syncing
    (new device should have no files, local work should be empty, server work should include root)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn new_unsynced_device() {
    for mut ops in [
        // unmodified
        vec![
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &[""]);
                },
            },
        ],
        // new_file
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &["", "document"]);
                },
            },
        ],
        // new_files
        vec![
            Create { client_num: 0, path: "a/b/c/d" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &["", "document"]);
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(
                        db,
                        root,
                        &["", "folder/", "folder/document"],
                    );
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &["", "document2"]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Delete { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &[""]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Delete { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &[""]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 0, path: "grandparent/" },
            Sync { client_num: 0 },
            Client { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_server_work_paths(db, root, &[""]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[1];
                db.validate().unwrap();
                test_utils::assert_all_paths(db, root, &[]);
                test_utils::assert_all_document_contents(db, root, &[]);
                test_utils::assert_local_work_paths(db, root, &[]);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device, sync it, then create and sync a new device
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn new_synced_device() {
    for mut ops in [
        // unmodified
        vec![
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // new_file
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                },
            },
        ],
        // new_files
        vec![
            Create { client_num: 0, path: "a/b/c/d" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a/b/c/d", b"")]);
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document", b"document content")],
                    );
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "folder/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("folder/document", b"")]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Delete { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Delete { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 0, path: "grandparent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[0];
                let db2 = &dbs[1];
                db.validate().unwrap();
                test_utils::assert_dbs_eq(db, db2);
                test_utils::assert_local_work_paths(db, root, &[]);
                test_utils::assert_server_work_paths(db, root, &[]);
                test_utils::assert_deleted_files_pruned(db);
                test_utils::assert_new_synced_client_dbs_eq(db);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that setup two synced devices, operate on one device, and sync it without syncing the other device
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn unsynced_change_new_synced_device() {
    for mut ops in [
        // unmodified
        vec![
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                },
            },
        ],
        // new_file
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &["document"]);
                },
            },
        ],
        // new_files
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a/b/c/d" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                    test_utils::assert_server_work_paths(
                        db,
                        root,
                        &["a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_server_work_paths(db, root, &["document"]);
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_server_work_paths(db, root, &["document"]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_server_work_paths(db, root, &["document"]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                    test_utils::assert_server_work_paths(db, root, &["document"]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("parent/document", b"")]);
                    test_utils::assert_server_work_paths(db, root, &["parent/", "parent/document"]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Sync { client_num: 0 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "grandparent/", "grandparent/parent/", "grandparent/parent/document"],
                    );
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("grandparent/parent/document", b"")],
                    );
                    test_utils::assert_server_work_paths(
                        db,
                        root,
                        &["grandparent/", "grandparent/parent/", "grandparent/parent/document"],
                    );
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[1];
                db.validate().unwrap();
                test_utils::assert_local_work_paths(db, root, &[]);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that setup two synced devices, operate on one device, and sync both
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn synced_change_new_synced_device() {
    for mut ops in [
        // unmodified
        vec![
            Sync { client_num: 1 },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // new_file
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                },
            },
        ],
        // new_files
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a/b/c/d" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a/b/", "a/b/c/", "a/b/c/d"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a/b/c/d", b"")]);
                },
            },
        ],
        // edited_document
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document", b"document content")],
                    );
                },
            },
        ],
        // move
        vec![
            Create { client_num: 0, path: "folder/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "folder/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "folder/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("folder/document", b"")]);
                },
            },
        ],
        // rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                },
            },
        ],
        // delete
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        ops.push(Custom {
            f: &|dbs, root| {
                let db = &dbs[0];
                let db2 = &dbs[1];
                db.validate().unwrap();
                test_utils::assert_dbs_eq(db, db2);
                test_utils::assert_local_work_paths(db, root, &[]);
                test_utils::assert_server_work_paths(db, root, &[]);
                test_utils::assert_deleted_files_pruned(db);
            },
        });
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that setup two synced devices, operate on both devices, then sync both twice
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn concurrent_change() {
    for mut ops in [
        // identical_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "parent/" },
            Move { client_num: 1, path: "document", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document"]);
                    test_utils::assert_all_document_contents(db, root, &[("parent/document", b"")]);
                },
            },
        ],
        // different_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "parent2/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "parent/" },
            Move { client_num: 1, path: "document", new_parent_path: "parent2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "parent/", "parent2/", "parent/document"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("parent/document", b"")]);
                },
            },
        ],
        // identical_rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Rename { client_num: 1, path: "document", new_name: "document2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                },
            },
        ],
        // different_rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Rename { client_num: 1, path: "document", new_name: "document3" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2"]);
                    test_utils::assert_all_document_contents(db, root, &[("document2", b"")]);
                },
            },
        ],
        // move_then_rename
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "parent/" },
            Rename { client_num: 1, path: "document", new_name: "document2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document2"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document2", b"")],
                    );
                },
            },
        ],
        // rename_then_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Move { client_num: 1, path: "document", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document2"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document2", b"")],
                    );
                },
            },
        ],
        // identical_delete
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document" },
            Delete { client_num: 1, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // identical_delete_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent_then_direct
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Delete { client_num: 1, path: "parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_direct_then_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/document" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // identical_delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Delete { client_num: 1, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent_then_direct
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Delete { client_num: 1, path: "grandparent/parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_direct_then_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 1, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent_then_parent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Delete { client_num: 1, path: "grandparent/parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent_then_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/parent/" },
            Delete { client_num: 1, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // move_then_delete
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "parent/" },
            Delete { client_num: 1, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_then_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document" },
            Move { client_num: 1, path: "document", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // move_then_delete_new_parent
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document", new_parent_path: "parent/" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_new_parent_then_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Move { client_num: 1, path: "document", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // move_then_delete_old_parent
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "parent/document", new_parent_path: "" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document"]);
                    test_utils::assert_all_document_contents(db, root, &[("document", b"")]);
                },
            },
        ],
        // delete_old_parent_then_move
        vec![
            Create { client_num: 0, path: "parent/document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Move { client_num: 1, path: "parent/document", new_parent_path: "" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // rename_then_delete
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document", new_name: "document2" },
            Delete { client_num: 1, path: "document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_then_rename
        vec![
            Create { client_num: 0, path: "document" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document" },
            Rename { client_num: 1, path: "document", new_name: "document2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // create_then_move_parent
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "parent2/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "parent/document" },
            Move { client_num: 1, path: "parent/", new_parent_path: "parent2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "parent2/", "parent2/parent/", "parent2/parent/document"],
                    );
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent2/parent/document", b"")],
                    );
                },
            },
        ],
        // move_parent_then_create
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "parent2/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "parent/", new_parent_path: "parent2/" },
            Create { client_num: 1, path: "parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "parent2/", "parent2/parent/", "parent2/parent/document"],
                    );
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent2/parent/document", b"")],
                    );
                },
            },
        ],
        // create_then_rename_parent
        vec![
            Create { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "parent/document" },
            Rename { client_num: 1, path: "parent/", new_name: "parent2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent2/", "parent2/document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent2/document", b"")],
                    );
                },
            },
        ],
        // rename_parent_then_create
        vec![
            Create { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "parent/", new_name: "parent2" },
            Create { client_num: 1, path: "parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent2/", "parent2/document"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent2/document", b"")],
                    );
                },
            },
        ],
        // create_then_delete_parent
        vec![
            Create { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "parent/document" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent_then_create
        vec![
            Create { client_num: 0, path: "parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Create { client_num: 1, path: "parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // create_then_delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "grandparent/parent/document" },
            Delete { client_num: 1, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent_then_create
        vec![
            Create { client_num: 0, path: "grandparent/parent/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Create { client_num: 1, path: "grandparent/parent/document" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // identical_content_edit_not_mergable
        vec![
            Create { client_num: 0, path: "document.draw" },
            Edit { client_num: 0, path: "document.draw", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.draw", content: b"document content 2" },
            Edit { client_num: 1, path: "document.draw", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "document.draw", "document-1.draw"],
                    );
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[
                            ("document.draw", b"document content 2"),
                            ("document-1.draw", b"document content 2"),
                        ],
                    );
                },
            },
        ],
        // identical_content_edit_mergable
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document content 2" },
            Edit { client_num: 1, path: "document.md", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document.md", b"document content 2")],
                    );
                },
            },
        ],
        // different_content_edit_not_mergable
        vec![
            Create { client_num: 0, path: "document.draw" },
            Edit { client_num: 0, path: "document.draw", content: b"document\n\ncontent\n" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.draw", content: b"document 2\n\ncontent\n" },
            Edit { client_num: 1, path: "document.draw", content: b"document\n\ncontent 2\n" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "document.draw", "document-1.draw"],
                    );
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[
                            ("document.draw", b"document 2\n\ncontent\n"),
                            ("document-1.draw", b"document\n\ncontent 2\n"),
                        ],
                    );
                },
            },
        ],
        // different_content_edit_mergable
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document\n\ncontent\n" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document 2\n\ncontent\n" },
            Edit { client_num: 1, path: "document.md", content: b"document\n\ncontent 2\n" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document.md", b"document 2\n\ncontent 2\n")],
                    );
                },
            },
        ],
        // different_content_edit_mergable_with_move_in_first_sync
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document\n\ncontent\n" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document 2\n\ncontent\n" },
            Move { client_num: 0, path: "document.md", new_parent_path: "parent/" },
            Edit { client_num: 1, path: "document.md", content: b"document\n\ncontent 2\n" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document.md", b"document 2\n\ncontent 2\n")],
                    );
                },
            },
        ],
        // different_content_edit_mergable_with_move_in_second_sync
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document\n\ncontent\n" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document 2\n\ncontent\n" },
            Edit { client_num: 1, path: "document.md", content: b"document\n\ncontent 2\n" },
            Move { client_num: 1, path: "document.md", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document.md", b"document 2\n\ncontent 2\n")],
                    );
                },
            },
        ],
        // move_then_edit_content
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "document.md", new_parent_path: "parent/" },
            Edit { client_num: 1, path: "document.md", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document.md", b"document content 2")],
                    );
                },
            },
        ],
        // edit_content_then_move
        vec![
            Create { client_num: 0, path: "parent/" },
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document content 2" },
            Move { client_num: 1, path: "document.md", new_parent_path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "parent/", "parent/document.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("parent/document.md", b"document content 2")],
                    );
                },
            },
        ],
        // rename_then_edit_content
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "document.md", new_name: "document2.md" },
            Edit { client_num: 1, path: "document.md", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document2.md", b"document content 2")],
                    );
                },
            },
        ],
        // edit_content_then_rename
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document content 2" },
            Rename { client_num: 1, path: "document.md", new_name: "document2.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "document2.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("document2.md", b"document content 2")],
                    );
                },
            },
        ],
        // delete_then_edit_content
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "document.md" },
            Edit { client_num: 1, path: "document.md", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // edit_content_then_delete
        vec![
            Create { client_num: 0, path: "document.md" },
            Edit { client_num: 0, path: "document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "document.md", content: b"document content 2" },
            Delete { client_num: 1, path: "document.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_parent_then_edit_content
        vec![
            Create { client_num: 0, path: "parent/document.md" },
            Edit { client_num: 0, path: "parent/document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "parent/" },
            Edit { client_num: 1, path: "parent/document.md", content: b"document content 2" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // edit_content_then_delete_parent
        vec![
            Create { client_num: 0, path: "parent/document.md" },
            Edit { client_num: 0, path: "parent/document.md", content: b"document content" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit { client_num: 0, path: "parent/document.md", content: b"document content 2" },
            Delete { client_num: 1, path: "parent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // delete_grandparent_then_edit_content
        vec![
            Create { client_num: 0, path: "grandparent/parent/document.md" },
            Edit {
                client_num: 0,
                path: "grandparent/parent/document.md",
                content: b"document content",
            },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Delete { client_num: 0, path: "grandparent/" },
            Edit {
                client_num: 1,
                path: "grandparent/parent/document.md",
                content: b"document content 2",
            },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // edit_content_then_delete_grandparent
        vec![
            Create { client_num: 0, path: "grandparent/parent/document.md" },
            Edit {
                client_num: 0,
                path: "grandparent/parent/document.md",
                content: b"document content",
            },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Edit {
                client_num: 0,
                path: "grandparent/parent/document.md",
                content: b"document content 2",
            },
            Delete { client_num: 1, path: "grandparent/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        let checks = ops.pop().unwrap();
        ops.extend(vec![
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    let db2 = &dbs[1];
                    db.validate().unwrap();
                    test_utils::assert_dbs_eq(db, db2);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                    test_utils::assert_deleted_files_pruned(db);
                },
            },
            checks,
        ]);
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests which are constructed to test cycle resolution
    Like those above, these are tests that setup two synced devices, operate on both devices, then sync both twice
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn cycle_resolution() {
    for mut ops in [
        // two_cycle
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "b/a/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c/", "c/b/", "c/b/a/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "b/a/", "c/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "d/", "d/c/", "d/c/b/", "d/c/b/a/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c/", "c/b/", "c/b/a/", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "b/a/", "d/", "d/c/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "b/a/", "c/", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // two_cycle_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 1, path: "b/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 0, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c2/", "c2/b2/", "c2/b2/a2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "c2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Rename { client_num: 0, path: "d/", new_name: "d2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 0, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 0, path: "c2/", new_parent_path: "d2/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "d2/", "d2/c2/", "d2/c2/b2/", "d2/c2/b2/a2/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Rename { client_num: 0, path: "d/", new_name: "d2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 0, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "c2/", "c2/b2/", "c2/b2/a2/", "d2/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Rename { client_num: 0, path: "d/", new_name: "d2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c2/", new_parent_path: "d2/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "d2/", "d2/c2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted_with_renames_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Rename { client_num: 0, path: "a/", new_name: "a2" },
            Rename { client_num: 0, path: "b/", new_name: "b2" },
            Rename { client_num: 0, path: "c/", new_name: "c2" },
            Rename { client_num: 0, path: "d/", new_name: "d2" },
            Move { client_num: 0, path: "a2/", new_parent_path: "b2/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "c2/", "d2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // two_cycle_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Move { client_num: 1, path: "b2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Move { client_num: 1, path: "c2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c2/", "c2/b2/", "c2/b2/a2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Move { client_num: 1, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 1, path: "c2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "c2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Rename { client_num: 1, path: "d/", new_name: "d2" },
            Move { client_num: 1, path: "d2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "d2/", "d2/c2/", "d2/c2/b2/", "d2/c2/b2/a2/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Rename { client_num: 1, path: "d/", new_name: "d2" },
            Move { client_num: 1, path: "c2/", new_parent_path: "d2/" },
            Move { client_num: 1, path: "d2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "c2/", "c2/b2/", "c2/b2/a2/", "d2/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Rename { client_num: 1, path: "d/", new_name: "d2" },
            Move { client_num: 1, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "d2/", "d2/c2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted_with_renames_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Rename { client_num: 1, path: "a/", new_name: "a2" },
            Rename { client_num: 1, path: "b/", new_name: "b2" },
            Rename { client_num: 1, path: "c/", new_name: "c2" },
            Rename { client_num: 1, path: "d/", new_name: "d2" },
            Move { client_num: 1, path: "b2/", new_parent_path: "c2/" },
            Move { client_num: 1, path: "c2/", new_parent_path: "d2/" },
            Move { client_num: 1, path: "d2/", new_parent_path: "a2/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b2/", "b2/a2/", "c2/", "d2/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // two_cycle_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "b/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "c/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "b/" },
            Delete { client_num: 0, path: "c/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "d/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "c/" },
            Delete { client_num: 0, path: "d/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "b/" },
            Delete { client_num: 0, path: "d/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted_with_deletes_first_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 0, path: "b/" },
            Delete { client_num: 0, path: "c/" },
            Delete { client_num: 0, path: "d/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &[""]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // two_cycle_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Delete { client_num: 1, path: "b/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "c/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Delete { client_num: 1, path: "b/" },
            Delete { client_num: 1, path: "c/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Delete { client_num: 1, path: "b/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "c/", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Delete { client_num: 1, path: "c/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted_with_deletes_second_device
        vec![
            Create { client_num: 0, path: "a/" },
            Create { client_num: 0, path: "b/" },
            Create { client_num: 0, path: "c/" },
            Create { client_num: 0, path: "d/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Delete { client_num: 1, path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "b/", "c/", "d/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // move_two_cycle_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "b/", "b/a/", "b/child/", "b/a/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_one_move_reverted_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "c/", "c/b/", "c/b/a/", "c/child/", "c/b/child/", "c/b/a/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // three_cycle_two_moves_reverted_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "b/", "b/a/", "c/", "b/child/", "b/a/child/", "c/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_one_move_reverted_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Create { client_num: 0, path: "d/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &[
                            "",
                            "d/",
                            "d/c/",
                            "d/c/b/",
                            "d/c/b/a/",
                            "d/child/",
                            "d/c/child/",
                            "d/c/b/child/",
                            "d/c/b/a/child/",
                        ],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_adjacent_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Create { client_num: 0, path: "d/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 0, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &[
                            "",
                            "c/",
                            "c/b/",
                            "c/b/a/",
                            "d/",
                            "c/child/",
                            "c/b/child/",
                            "c/b/a/child/",
                            "d/child/",
                        ],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_two_moves_reverted_alternating_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Create { client_num: 0, path: "d/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 0, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &[
                            "",
                            "b/",
                            "b/a/",
                            "d/",
                            "d/c/",
                            "b/child/",
                            "b/a/child/",
                            "d/child/",
                            "d/c/child/",
                        ],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // four_cycle_three_moves_reverted_with_children
        vec![
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 0, path: "b/child/" },
            Create { client_num: 0, path: "c/child/" },
            Create { client_num: 0, path: "d/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "a/", new_parent_path: "b/" },
            Move { client_num: 1, path: "b/", new_parent_path: "c/" },
            Move { client_num: 1, path: "c/", new_parent_path: "d/" },
            Move { client_num: 1, path: "d/", new_parent_path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &[
                            "",
                            "b/",
                            "b/a/",
                            "c/",
                            "d/",
                            "b/child/",
                            "b/a/child/",
                            "c/child/",
                            "d/child/",
                        ],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        let checks = ops.pop().unwrap();
        ops.extend(vec![
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    let db2 = &dbs[1];
                    db.validate().unwrap();
                    test_utils::assert_dbs_eq(db, db2);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                    test_utils::assert_deleted_files_pruned(db);
                },
            },
            checks,
        ]);
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests which are constructed to test path conflict resolution
    Like those above, these are tests that setup two synced devices, operate on both devices, then sync both twice
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn path_conflict_resolution() {
    for mut ops in [
        // concurrent_create_documents
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md" },
            Create { client_num: 1, path: "a.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "a.md", "a-1.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("a.md", b""), ("a-1.md", b"")],
                    );
                },
            },
        ],
        // concurrent_create_folders
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a/" },
            Create { client_num: 1, path: "a/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "a/", "a-1/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // concurrent_create_folders_with_children
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a/child/" },
            Create { client_num: 1, path: "a/child/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a/", "a-1/", "a/child/", "a-1/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // concurrent_create_document_then_folder
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md" },
            Create { client_num: 1, path: "a.md/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "a.md", "a-1.md/"]);
                    test_utils::assert_all_document_contents(db, root, &[("a.md", b"")]);
                },
            },
        ],
        // concurrent_create_folder_then_document
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md/" },
            Create { client_num: 1, path: "a.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "a.md/", "a-1.md"]);
                    test_utils::assert_all_document_contents(db, root, &[("a-1.md", b"")]);
                },
            },
        ],
        // concurrent_create_document_then_folder_with_child
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md" },
            Create { client_num: 1, path: "a.md/child/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "a.md", "a-1.md/", "a-1.md/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[("a.md", b"")]);
                },
            },
        ],
        // concurrent_create_folder_with_child_then_document
        vec![
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md/child/" },
            Create { client_num: 1, path: "a.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "a.md/", "a.md/child/", "a-1.md"]);
                    test_utils::assert_all_document_contents(db, root, &[("a-1.md", b"")]);
                },
            },
        ],
        // concurrent_move_then_create_documents
        vec![
            Create { client_num: 0, path: "folder/a.md" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "folder/a.md", new_parent_path: "" },
            Create { client_num: 1, path: "a.md" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "a.md", "a-1.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("a.md", b""), ("a-1.md", b"")],
                    );
                },
            },
        ],
        // concurrent_create_then_move_documents
        vec![
            Create { client_num: 0, path: "folder/a.md" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md" },
            Move { client_num: 1, path: "folder/a.md", new_parent_path: "" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "a.md", "a-1.md"]);
                    test_utils::assert_all_document_contents(
                        db,
                        root,
                        &[("a.md", b""), ("a-1.md", b"")],
                    );
                },
            },
        ],
        // concurrent_move_then_create_folders
        vec![
            Create { client_num: 0, path: "folder/a.md/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "folder/a.md/", new_parent_path: "" },
            Create { client_num: 1, path: "a.md/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "a.md/", "a-1.md/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // concurrent_create_then_move_folders
        vec![
            Create { client_num: 0, path: "folder/a.md/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md/" },
            Move { client_num: 1, path: "folder/a.md/", new_parent_path: "" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(db, root, &["", "folder/", "a.md/", "a-1.md/"]);
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // concurrent_move_then_create_folders_with_children
        vec![
            Create { client_num: 0, path: "folder/a.md/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Move { client_num: 0, path: "folder/a.md/", new_parent_path: "" },
            Create { client_num: 1, path: "a.md/child/" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "folder/", "a.md/", "a-1.md/", "a.md/child/", "a-1.md/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
        // concurrent_create_then_move_folders_with_children
        vec![
            Create { client_num: 0, path: "folder/a.md/child/" },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Create { client_num: 0, path: "a.md/child/" },
            Move { client_num: 1, path: "folder/a.md/", new_parent_path: "" },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[1];
                    test_utils::assert_all_paths(
                        db,
                        root,
                        &["", "folder/", "a.md/", "a-1.md/", "a.md/child/", "a-1.md/child/"],
                    );
                    test_utils::assert_all_document_contents(db, root, &[]);
                },
            },
        ],
    ] {
        let checks = ops.pop().unwrap();
        ops.extend(vec![
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Sync { client_num: 0 },
            Sync { client_num: 1 },
            Custom {
                f: &|dbs, root| {
                    let db = &dbs[0];
                    let db2 = &dbs[1];
                    db.validate().unwrap();
                    test_utils::assert_dbs_eq(db, db2);
                    test_utils::assert_local_work_paths(db, root, &[]);
                    test_utils::assert_server_work_paths(db, root, &[]);
                    test_utils::assert_deleted_files_pruned(db);
                },
            },
            checks,
        ]);
        test_utils::run(&ops);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Uncategorized tests
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn test_path_conflict() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path(&path(&db1, "new.md")).unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path(&path(&db2, "new.md")).unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.decrypted_name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new.md"]
    )
}

#[test]
fn test_path_conflict2() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path(&path(&db1, "new-1.md")).unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path(&path(&db2, "new-1.md")).unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.decrypted_name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new-2.md"]
    )
}

#[test]
fn deleted_path_is_released() {
    let db1 = test_core_with_account();
    let file1 = db1.create_at_path(&path(&db1, "file1.md")).unwrap();
    db1.sync(None).unwrap();

    db1.db
        .transaction(|tx| {
            tx.insert_metadatum(
                &db1.config,
                RepoSource::Local,
                &files::apply_delete(&tx.get_all_metadata(RepoSource::Local).unwrap(), file1.id)
                    .unwrap(),
            )
        })
        .unwrap()
        .unwrap();

    db1.sync(None).unwrap();
    db1.create_at_path(&path(&db1, "file1.md")).unwrap();
    db1.sync(None).unwrap();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test() {
    let db1 = test_core_with_account();
    let b = db1.create_at_path(&path(&db1, "b")).unwrap();
    let c = db1.create_at_path(&path(&db1, "c/")).unwrap();
    let d = db1.create_at_path(&path(&db1, "c/d/")).unwrap();
    db1.move_file(b.id, d.id).unwrap();
    db1.move_file(c.id, d.id).unwrap_err();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_2() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let a = db2.create_at_path(&path(&db2, "a/")).unwrap();
    let b = db2.create_at_path(&path(&db2, "a/b/")).unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let _c = db2.create_at_path(&path(&db2, "c/")).unwrap();
    db2.move_file(b.id, a.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_3() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let _a = db2.create_at_path(&path(&db2, "a/")).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);

    db1.create_at_path(&path(&db1, "a/b.md")).unwrap();
    let c = db1.create_at_path(&path(&db1, "a/c")).unwrap();
    db1.rename_file(c.id, "c2").unwrap();

    db1.create_at_path(&path(&db1, "a/d")).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_4() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let _a = db2.create_at_path(&path(&db2, "a/")).unwrap();
    let b = db2.create_at_path(&path(&db2, "a/b/")).unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let c = db2.create_at_path(&path(&db2, "c.md")).unwrap();
    db2.write_document(c.id, b"DPCN8G0CK8qXSyJhervmmEXFnkt")
        .unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);
}

#[test]
fn fuzzer_stuck_test_5() {
    let db1 = test_core_with_account();
    let root = db1.get_root().unwrap();
    let db2 = test_core_from(&db1);

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();

    let a = db1.create_at_path(&path(&db1, "a/")).unwrap();
    let b = db1.create_at_path(&path(&db1, "a/b/")).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);

    db1.move_file(b.id, root.id).unwrap();
    db1.move_file(a.id, b.id).unwrap();
    db1.delete_file(b.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    test_utils::assert_dbs_eq(&db1, &db2);
}

#[test]
fn fuzzer_stuck_test_6() {
    let core1 = test_core_with_account();

    let dir1 = core1.create_at_path(&path(&core1, "quB/")).unwrap();
    let dir2 = core1.create_at_path(&path(&core1, "OO1/")).unwrap();
    core1.sync(None).unwrap();
    let core2 = test_core_from(&core1);
    core2.move_file(dir2.id, dir1.id).unwrap();
    let _doc1 = core1.create_at_path(&path(&core1, "KbW")).unwrap();
    core1.move_file(dir1.id, dir2.id).unwrap();

    core1.sync(None).unwrap();
    println!("v----------------------------------------------------------v");
    core2.sync(None).unwrap();
    println!("^----------------------------------------------------------^");
    // core1.sync(None).unwrap();
    // core2.sync(None).unwrap();
    // core1.validate().unwrap();
    // test_utils::assert_dbs_eq(&core1, &core2);
}

#[test]
fn fuzzer_get_updates_required_test() {
    let db1 = test_core_with_account();

    let document = db1.create_at_path(&path(&db1, "document")).unwrap();

    db1.sync(None).unwrap();
    let db2 = test_core_from(&db1);

    db1.write_document(document.id, b"document content")
        .unwrap();
    db2.write_document(document.id, b"content document")
        .unwrap();
    db2.delete_file(document.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
}

#[test]
fn fuzzer_new_file_deleted() {
    let core = test_core_with_account();

    let dir1 = core.create_at_path(&path(&core, "u88/")).unwrap();
    core.sync(None).unwrap();
    let dir2 = core.create_at_path(&path(&core, "mep/")).unwrap();
    core.move_file(dir1.id, dir2.id).unwrap();
    core.delete_file(dir2.id).unwrap();
    core.sync(None).unwrap();
}
