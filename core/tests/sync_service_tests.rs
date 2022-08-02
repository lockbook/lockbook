use hmdb::transaction::Transaction;
use itertools::Itertools;
use lockbook_core::Core;
use test_utils::*;

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device after syncing
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn unsynced_change_synced_device() {
    // new_file
    {
        let core = test_core_with_account();
        core.sync(None).unwrap();
        core.create_at_path("/document").unwrap();
        assert_all_paths(&core, &["/", "/document"]);
        assert_all_document_contents(&core, &[("/document", b"")]);
        assert_local_work_paths(&core, &["/document"]);
        core.validate().unwrap();
        assert_server_work_paths(&core, &[]);
    }

    // new_files
    {
        let core = test_core_with_account();
        core.sync(None).unwrap();
        core.create_at_path("/a/b/c/d").unwrap();
        assert_all_paths(&core, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        assert_all_document_contents(&core, &[("/a/b/c/d", b"")]);
        assert_local_work_paths(&core, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        core.validate().unwrap();
        assert_server_work_paths(&core, &[]);
    }

    // edited_document
    {
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

    // edit_unedit
    {
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

    // move
    {
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

    // move_unmove
    {
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

    // rename
    {
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

    // rename_unrename
    {
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

    // delete
    {
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

    // delete_parent
    {
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

    // delete_grandparent
    {
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
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device, sync it, then create a new device without syncing
    (new device should have no files, local work should be empty, server work should include root)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn new_unsynced_device() {
    let assert_stuff = |c: &Core| {
        c.validate().unwrap();
        assert_all_paths(c, &[]);
        assert_all_document_contents(c, &[]);
        assert_local_work_paths(c, &[]);
    };

    // unmodified
    {
        let c1 = test_core_with_account();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/"]);
        assert_stuff(&c2);
    }

    // new_file
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/", "/document"]);
        assert_stuff(&c2);
    }

    // new_files
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/a/b/c/d").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        assert_stuff(&c2);
    }

    // edited_document
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        write_path(&c1, "/document", b"document content").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/", "/document"]);
        assert_stuff(&c2);
    }

    // move
    {
        let c1 = test_core_with_account();
        let folder = c1.create_at_path("/folder/").unwrap();
        let doc = c1.create_at_path("/document").unwrap();
        c1.move_file(doc.id, folder.id).unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/", "/folder/", "/folder/document"]);
        assert_stuff(&c2);
    }

    // rename
    {
        let c1 = test_core_with_account();
        let doc = c1.create_at_path("/document").unwrap();
        c1.rename_file(doc.id, "document2").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/", "/document2"]);
        assert_stuff(&c2);
    }

    // delete
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        delete_path(&c1, "/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/"]);
        assert_stuff(&c2);
    }

    // delete_parent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/parent/document").unwrap();
        delete_path(&c1, "/parent/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/"]);
        assert_stuff(&c2);
    }

    // delete_grandparent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/grandparent/parent/document").unwrap();
        delete_path(&c1, "/grandparent/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        assert_server_work_paths(&c2, &["/"]);
        assert_stuff(&c2);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that operate on one device, sync it, then create and sync a new device
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn new_synced_device() {
    let assert_stuff = |c1: &Core, c2: &Core| {
        c1.validate().unwrap();
        assert_dbs_eq(c1, c2);
        assert_local_work_paths(c1, &[]);
        assert_server_work_paths(c1, &[]);
        assert_deleted_files_pruned(c1);
        assert_new_synced_client_dbs_eq(c1);
    };

    // unmodified
    {
        let c1 = test_core_with_account();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // new_file
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_stuff(&c1, &c2);
    }

    // new_files
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/a/b/c/d").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        assert_all_document_contents(&c2, &[("/a/b/c/d", b"")]);
        assert_stuff(&c1, &c2);
    }

    // edited_document
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        write_path(&c1, "/document", b"document content").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"document content")]);
        assert_stuff(&c1, &c2);
    }

    // move
    {
        let c1 = test_core_with_account();
        let folder = c1.create_at_path("/folder/").unwrap();
        let doc = c1.create_at_path("/document").unwrap();
        c1.move_file(doc.id, folder.id).unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/folder/", "/folder/document"]);
        assert_all_document_contents(&c2, &[("/folder/document", b"")]);
        assert_stuff(&c1, &c2);
    }

    // rename
    {
        let c1 = test_core_with_account();
        let doc = c1.create_at_path("/document").unwrap();
        c1.rename_file(doc.id, "document2").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/document2"]);
        assert_all_document_contents(&c2, &[("/document2", b"")]);
        assert_stuff(&c1, &c2);
    }

    // delete
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        delete_path(&c1, "/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // delete_parent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/parent/document").unwrap();
        delete_path(&c1, "/parent/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // delete_grandparent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/grandparent/parent/document").unwrap();
        delete_path(&c1, "/grandparent/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that setup two synced devices, operate on one device, and sync it without syncing the other device
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn unsynced_change_new_synced_device() {
    let assert_stuff = |c: &Core| {
        c.validate().unwrap();
        assert_local_work_paths(&c, &[]);
    };

    // unmodified
    {
        let c1 = test_core_with_account();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_server_work_paths(&c2, &[]);
        assert_stuff(&c2);
    }

    // new_file
    {
        let c1 = test_core_with_account();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_server_work_paths(&c2, &["/document"]);
        assert_stuff(&c2);
    }

    // new_files
    {
        let c1 = test_core_with_account();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/a/b/c/d").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_server_work_paths(&c2, &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        assert_stuff(&c2);
    }

    // edited_document
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        write_path(&c1, "/document", b"document content").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_server_work_paths(&c2, &["/document"]);
        assert_stuff(&c2);
    }

    // move
    {
        let c1 = test_core_with_account();
        let folder = c1.create_at_path("/folder/").unwrap();
        let doc = c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.move_file(doc.id, folder.id).unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/", "/folder/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_server_work_paths(&c2, &["/document"]);
        assert_stuff(&c2);
    }

    // rename
    {
        let c1 = test_core_with_account();
        let doc = c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.rename_file(doc.id, "document2").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_server_work_paths(&c2, &["/document"]);
        assert_stuff(&c2);
    }

    // delete
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/document").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_server_work_paths(&c2, &["/document"]);
        assert_stuff(&c2);
    }

    // delete_parent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/parent/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/parent/").unwrap();
        c1.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/parent/", "/parent/document"]);
        assert_all_document_contents(&c2, &[("/parent/document", b"")]);
        assert_server_work_paths(&c2, &["/parent/", "/parent/document"]);
        assert_stuff(&c2);
    }

    // delete_grandparent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/grandparent/parent/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/grandparent/").unwrap();
        c1.sync(None).unwrap();

        assert_all_paths(
            &c2,
            &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
        );
        assert_all_document_contents(&c2, &[("/grandparent/parent/document", b"")]);
        assert_server_work_paths(
            &c2,
            &["/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
        );
        assert_stuff(&c2);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests that setup two synced devices, operate on one device, and sync both
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn synced_change_new_synced_device() {
    let assert_stuff = |c1: &Core, c2: &Core| {
        c1.validate().unwrap();
        assert_dbs_eq(c1, c2);
        assert_local_work_paths(c1, &[]);
        assert_server_work_paths(c1, &[]);
        assert_deleted_files_pruned(c1);
    };

    // unmodified
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // new_file
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"")]);
        assert_stuff(&c1, &c2);
    }

    // new_files
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/a/b/c/d").unwrap();
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        assert_all_document_contents(&c2, &[("/a/b/c/d", b"")]);
        assert_stuff(&c1, &c2);
    }

    // edited_document
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        write_path(&c1, "/document", b"document content").unwrap();
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();

        assert_all_paths(&c2, &["/", "/document"]);
        assert_all_document_contents(&c2, &[("/document", b"document content")]);
        assert_stuff(&c1, &c2);
    }

    // move
    {
        let c1 = test_core_with_account();
        let folder = c1.create_at_path("/folder/").unwrap();
        let doc = c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.move_file(doc.id, folder.id).unwrap();
        c1.sync(None).unwrap();

        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/folder/", "/folder/document"]);
        assert_all_document_contents(&c2, &[("/folder/document", b"")]);
        assert_stuff(&c1, &c2);
    }

    // rename
    {
        let c1 = test_core_with_account();
        let doc = c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.rename_file(doc.id, "document2").unwrap();
        c1.sync(None).unwrap();

        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/", "/document2"]);
        assert_all_document_contents(&c2, &[("/document2", b"")]);
        assert_stuff(&c1, &c2);
    }

    // delete
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/document").unwrap();
        c1.sync(None).unwrap();

        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // delete_parent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/parent/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/parent/").unwrap();
        c1.sync(None).unwrap();

        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }

    // delete_grandparent
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/grandparent/parent/document").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        delete_path(&c1, "/grandparent/").unwrap();
        c1.sync(None).unwrap();

        c2.sync(None).unwrap();
        assert_all_paths(&c2, &["/"]);
        assert_all_document_contents(&c2, &[]);
        assert_stuff(&c1, &c2);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Tests which are constructed to test path conflict resolution
    Like those above, these are tests that setup two synced devices, operate on both devices, then sync both twice
    (work should be none, devices dbs should be equal, deleted files should be pruned)
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn path_conflict_resolution() {
    let sync_and_assert_stuff = |c1: &Core, c2: &Core| {
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();
        c1.sync(None).unwrap();
        c2.sync(None).unwrap();

        c1.validate().unwrap();
        assert_dbs_eq(&c1, c2);
        assert_local_work_paths(&c1, &[]);
        assert_server_work_paths(&c1, &[]);
        assert_deleted_files_pruned(&c1);
    };

    // concurrent_create_documents
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a.md").unwrap();
        c2.create_at_path("/a.md").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a.md", "/a-1.md"]);
        assert_all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
    }

    // concurrent_create_folders
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a/").unwrap();
        c2.create_at_path("/a/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a/", "/a-1/"]);
        assert_all_document_contents(&c2, &[]);
    }

    // concurrent_create_folders_with_children
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a/child/").unwrap();
        c2.create_at_path("/a/child/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a/", "/a-1/", "/a/child/", "/a-1/child/"]);
        assert_all_document_contents(&c2, &[]);
    }

    // concurrent_create_document_then_folder
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a.md").unwrap();
        c2.create_at_path("/a.md/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a.md", "/a-1.md/"]);
        assert_all_document_contents(&c2, &[("/a.md", b"")]);
    }

    // concurrent_create_folder_then_document
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a.md/").unwrap();
        c2.create_at_path("/a.md").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a.md/", "/a-1.md"]);
        assert_all_document_contents(&c2, &[("/a-1.md", b"")]);
    }

    // concurrent_create_document_then_folder_with_child
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a.md").unwrap();
        c2.create_at_path("/a.md/child/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a.md", "/a-1.md/", "/a-1.md/child/"]);
        assert_all_document_contents(&c2, &[("/a.md", b"")]);
    }

    // concurrent_create_folder_with_child_then_document
    {
        let c1 = test_core_with_account();
        let c2 = another_client(&c1);
        c2.sync(None).unwrap();
        c1.create_at_path("/a.md/child/").unwrap();
        c2.create_at_path("/a.md").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/a.md/", "/a.md/child/", "/a-1.md"]);
        assert_all_document_contents(&c2, &[("/a-1.md", b"")]);
    }

    // concurrent_move_then_create_documents
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        move_by_path(&c1, "/folder/a.md", "").unwrap();
        c2.create_at_path("/a.md").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]);
        assert_all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
    }

    // concurrent_create_then_move_documents
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/a.md").unwrap();
        move_by_path(&c2, "/folder/a.md", "").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/folder/", "/a.md", "/a-1.md"]);
        assert_all_document_contents(&c2, &[("/a.md", b""), ("/a-1.md", b"")]);
    }

    // concurrent_move_then_create_folders
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        move_by_path(&c1, "/folder/a.md/", "").unwrap();
        c2.create_at_path("/a.md/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
        assert_all_document_contents(&c2, &[]);
    }

    // concurrent_create_then_move_folders
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/a.md/").unwrap();
        move_by_path(&c2, "/folder/a.md/", "").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(&c2, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
        assert_all_document_contents(&c2, &[]);
    }

    // concurrent_move_then_create_folders_with_children
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md/child/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        move_by_path(&c1, "/folder/a.md/", "").unwrap();
        c2.create_at_path("/a.md/child/").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(
            &c2,
            &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
        );
        assert_all_document_contents(&c2, &[]);
    }

    // concurrent_create_then_move_folders_with_children
    {
        let c1 = test_core_with_account();
        c1.create_at_path("/folder/a.md/child/").unwrap();
        c1.sync(None).unwrap();

        let c2 = another_client(&c1);
        c2.sync(None).unwrap();

        c1.create_at_path("/a.md/child/").unwrap();
        move_by_path(&c2, "/folder/a.md/", "").unwrap();

        sync_and_assert_stuff(&c1, &c2);
        assert_all_paths(
            &c2,
            &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
        );
        assert_all_document_contents(&c2, &[]);
    }
}

/*  ---------------------------------------------------------------------------------------------------------------
    Uncategorized tests
---------------------------------------------------------------------------------------------------------------  */

#[test]
fn test_path_conflict() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path("new.md").unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path("new.md").unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new.md"]
    )
}

#[test]
fn test_path_conflict2() {
    let db1 = test_core_with_account();
    let db2 = test_core_from(&db1);

    db1.create_at_path("new-1.md").unwrap();
    db1.sync(None).unwrap();
    db2.create_at_path("new-1.md").unwrap();
    db2.sync(None).unwrap();

    assert_eq!(
        db2.list_metadatas()
            .unwrap()
            .iter()
            .filter(|file| file.id != file.parent)
            .map(|file| file.name.clone())
            .sorted()
            .collect::<Vec<String>>(),
        ["new-1.md", "new-2.md"]
    )
}

#[test]
fn deleted_path_is_released() {
    let db1 = test_core_with_account();
    let file1 = db1.create_at_path("file1.md").unwrap();
    db1.sync(None).unwrap();

    db1.db
        .transaction(|tx| {
            let mut ctx = db1.context(tx).unwrap();
            ctx.delete(&file1.id).unwrap();
        })
        .unwrap();

    db1.sync(None).unwrap();
    db1.create_at_path("file1.md").unwrap();
    db1.sync(None).unwrap();

    let db2 = test_core_from(&db1);
    db2.sync(None).unwrap();
}

// this case did not actually get the fuzzer stuck and was written while reproducing the issue
#[test]
fn fuzzer_stuck_test_1() {
    let db1 = test_core_with_account();
    let b = db1.create_at_path("/b").unwrap();
    let c = db1.create_at_path("/c/").unwrap();
    let d = db1.create_at_path("/c/d/").unwrap();
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

    let a = db2.create_at_path("/a/").unwrap();
    let b = db2.create_at_path("/a/b/").unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let _c = db2.create_at_path("/c/").unwrap();
    db2.move_file(b.id, a.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);
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

    let _a = db2.create_at_path("/a/").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);

    db1.create_at_path("/a/b.md").unwrap();
    let c = db1.create_at_path("/a/c").unwrap();
    db1.rename_file(c.id, "c2").unwrap();

    db1.create_at_path("/a/d").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);
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

    let _a = db2.create_at_path("/a/").unwrap();
    let b = db2.create_at_path("/a/b/").unwrap();
    db2.move_file(b.id, root.id).unwrap();
    db2.rename_file(b.id, "b2").unwrap();
    let c = db2.create_at_path("c.md").unwrap();
    db2.write_document(c.id, b"DPCN8G0CK8qXSyJhervmmEXFnkt")
        .unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);
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

    let a = db1.create_at_path("/a/").unwrap();
    let b = db1.create_at_path("/a/b/").unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);

    db1.move_file(b.id, root.id).unwrap();
    db1.move_file(a.id, b.id).unwrap();
    db1.delete_file(b.id).unwrap();

    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.sync(None).unwrap();
    db2.sync(None).unwrap();
    db1.validate().unwrap();
    assert_dbs_eq(&db1, &db2);
}

#[test]
fn fuzzer_stuck_test_6() {
    let core1 = test_core_with_account();

    let dir1 = core1.create_at_path("quB/").unwrap();
    let dir2 = core1.create_at_path("OO1/").unwrap();
    core1.sync(None).unwrap();
    let core2 = test_core_from(&core1);
    core2.move_file(dir2.id, dir1.id).unwrap();
    let _doc1 = core1.create_at_path("KbW").unwrap();
    core1.move_file(dir1.id, dir2.id).unwrap();

    core1.sync(None).unwrap();
    core2.sync(None).unwrap();
    core1.sync(None).unwrap();
    core2.sync(None).unwrap();
    core1.validate().unwrap();
    assert_dbs_eq(&core1, &core2);
}

#[test]
fn fuzzer_get_updates_required_test() {
    let db1 = test_core_with_account();

    let document = db1.create_at_path("/document").unwrap();

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

    let dir1 = core.create_at_path("u88/").unwrap();
    core.sync(None).unwrap();
    let dir2 = core.create_at_path("mep/").unwrap();
    core.move_file(dir1.id, dir2.id).unwrap();
    core.delete_file(dir2.id).unwrap();
    core.sync(None).unwrap();
}
