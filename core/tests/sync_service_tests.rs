#[cfg(test)]
mod sync_tests {
    use itertools::Itertools;

    use lockbook_core::model::repo::RepoSource;
    use lockbook_core::pure_functions::files;
    use lockbook_core::service::test_utils::Operation;
    use lockbook_core::service::{file_service, test_utils};

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device without syncing
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn unsynced_device() {
        for mut ops in [
            // unmodified
            vec![Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[0].1;
                    test_utils::assert_all_paths(&db, &root, &["/"]);
                    test_utils::assert_all_document_contents(&db, &root, &[]);
                    test_utils::assert_local_work_paths(&db, &root, &[]);
                    test_utils::assert_server_work_paths(&db, &root, &[]);
                },
            }],
            // new_file
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, &root, &["/document"]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
                        test_utils::assert_local_work_paths(
                            &db,
                            &root,
                            &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // edited_document
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document", b"document content")],
                        );
                        test_utils::assert_local_work_paths(&db, &root, &["/document"]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // move
            vec![
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/folder/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/folder/document", b"")],
                        );
                        test_utils::assert_local_work_paths(
                            &db,
                            &root,
                            &["/folder/", "/folder/document"],
                        );
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2", b"")],
                        );
                        test_utils::assert_local_work_paths(&db, &root, &["/document2"]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, _| {
                    let db = &dbs[0].1;
                    test_utils::assert_repo_integrity(&db);
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
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // new_file
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                    },
                },
            ],
            // new_file_name_same_as_username
            vec![
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        let account = lockbook_core::get_account(&db).unwrap();
                        let document_path =
                            test_utils::path(&root, &format!("/{}", account.username));
                        lockbook_core::create_file_at_path(&db, &document_path).unwrap();
                    },
                },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        let account = lockbook_core::get_account(&db).unwrap();
                        let document_path = format!("/{}", account.username);
                        test_utils::assert_all_paths(&db, &root, &["/", &document_path]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[(&document_path, b"")],
                        );
                    },
                },
            ],
            // new_files
            vec![
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
                    },
                },
            ],
            // edited_document
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document", b"document content")],
                        );
                    },
                },
            ],
            // move
            vec![
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/folder/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/folder/document", b"")],
                        );
                    },
                },
            ],
            // rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2", b"")],
                        );
                    },
                },
            ],
            // delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/folder/document" },
                Operation::Delete { client_num: 0, path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[0].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_local_work_paths(&db, &root, &[]);
                    test_utils::assert_server_work_paths(&db, &root, &[]);
                    test_utils::assert_deleted_files_pruned(&db);
                    test_utils::assert_new_synced_client_dbs_eq(&db);
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
                Operation::Sync { client_num: 0 },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, root, &["/document"]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Sync { client_num: 0 },
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
                        test_utils::assert_local_work_paths(
                            &db,
                            root,
                            &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                    },
                },
            ],
            // edited_document
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document", b"document content")],
                        );
                        test_utils::assert_local_work_paths(&db, root, &["/document"]);
                    },
                },
            ],
            // edit_unedit
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Edit { client_num: 0, path: "/document", content: b"" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, root, &[]);
                    },
                },
            ],
            // move
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/folder/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/folder/document", b"")],
                        );
                        test_utils::assert_local_work_paths(&db, root, &["/folder/document"]);
                    },
                },
            ],
            // move_unmove
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, root, &[]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[0].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_server_work_paths(&db, &root, &[]);
                },
            });
            test_utils::run(&ops);
        }
    }

    #[test]
    fn unsynced_change_synced_device_move_unmove() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::move_file(&db, document.id, root.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_unrename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();
        lockbook_core::rename_file(&db, document.id, "document").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::delete_file(&db, document.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::delete_file(&db, parent.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[parent.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::delete_file(&db, grandparent.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[grandparent.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device, sync it, then create a new device without syncing
        (new device should have no files, local work should be empty, server work should include root)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn new_unsynced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id]);
    }

    #[test]
    fn new_unsynced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, document.id]);
    }

    #[test]
    fn new_unsynced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/")).unwrap();
        let c =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, a.id, b.id, c.id, d.id]);
    }

    #[test]
    fn new_unsynced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, document.id]);
    }

    #[test]
    fn new_unsynced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, folder.id, document.id]);
    }

    #[test]
    fn new_unsynced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, document.id]);
    }

    #[test]
    fn new_unsynced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::delete_file(&db, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id]);
    }

    #[test]
    fn new_unsynced_device_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::delete_file(&db, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id]);
    }

    #[test]
    fn new_unsynced_device_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();
        lockbook_core::delete_file(&db, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device, sync it, then create and sync a new device
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn new_synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::delete_file(&db, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::delete_file(&db, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn new_synced_device_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();
        lockbook_core::delete_file(&db, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that setup two synced devices, operate on one device, and sync it without syncing the other device
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn unsynced_change_new_synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
    }

    #[test]
    fn unsynced_change_new_synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/")).unwrap();
        let c =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[a.id, b.id, c.id, d.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/folder/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/parent/", "/parent/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/parent/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[parent.id, document.id]);
    }

    #[test]
    fn unsynced_change_new_synced_device_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(
            &db2,
            &root,
            &["/", "/grandparent/", "/grandparent/parent/", "/grandparent/parent/document"],
        );
        test_utils::assert_all_document_contents(
            &db2,
            &root,
            &[("/grandparent/parent/document", b"")],
        );
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[grandparent.id, parent.id, document.id]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that setup two synced devices, operate on one device, and sync both
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn synced_change_new_synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(
            &db2,
            &root,
            &[("/document", b"document content")],
        );
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    #[test]
    fn synced_change_new_synced_device_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
        test_utils::assert_deleted_files_pruned(&db2);
        test_utils::assert_new_synced_client_dbs_eq(&db2);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that setup two synced devices, operate on both devices, then sync both twice
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn concurrent_change_identical_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::move_file(&db2, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let folder2 =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder2/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::move_file(&db2, document.id, folder2.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/folder2/", "/folder/document"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();
        lockbook_core::rename_file(&db2, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();
        lockbook_core::rename_file(&db2, document.id, "document3").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_then_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::rename_file(&db2, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_rename_then_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::rename_file(&db2, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, document.id).unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_parent_then_direct() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_direct_then_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();
        lockbook_core::delete_file(&db2, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_grandparent_then_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_parent_then_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_then_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, parent.id).unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/parent/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_then_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, parent.id).unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/parent/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_then_delete_new_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, parent.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_new_parent_then_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, parent.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_then_delete_old_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, root.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_old_parent_then_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, root.id).unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_rename_then_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_then_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2").unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_create_then_move_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let parent2 =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent2/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::move_file(&db2, parent.id, parent2.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/parent2/parent/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_parent_then_create() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let parent2 =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent2/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::move_file(&db2, parent.id, parent2.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/parent2/parent/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_create_then_rename_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::rename_file(&db2, parent.id, "parent2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/parent2/", "/parent2/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/parent2/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_rename_parent_then_create() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::rename_file(&db2, parent.id, "parent2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/parent2/", "/parent2/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/parent2/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_create_then_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_parent_then_create() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/document"))
                .unwrap();
        lockbook_core::delete_file(&db2, parent.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_create_then_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();
        lockbook_core::delete_file(&db2, grandparent.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_grandparent_then_create() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document"),
        )
        .unwrap();
        lockbook_core::delete_file(&db2, grandparent.id).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_content_edit_not_mergable() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.draw"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document content 2").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document.draw", "/document-1.draw"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[
                ("/document.draw", b"document content 2"),
                ("/document-1.draw", b"document content 2"),
            ],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_identical_content_edit_mergable() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document content 2").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/document.md", b"document content 2")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_content_edit_not_mergable() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.draw"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document content 2").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 3").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document.draw", "/document-1.draw"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[
                ("/document.draw", b"document content 2"),
                ("/document-1.draw", b"document content 3"),
            ],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_content_edit_mergable() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document\n\ncontent\n").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document 2\n\ncontent\n").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document\n\ncontent 2\n").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/document.md", b"document 2\n\ncontent 2\n")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_content_edit_mergable_with_move_in_first_sync() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document\n\ncontent\n").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document 2\n\ncontent\n").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document\n\ncontent 2\n").unwrap();
        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/folder/document.md", b"document 2\n\ncontent 2\n")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_different_content_edit_mergable_with_move_in_second_sync() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document\n\ncontent\n").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::write_document(&db, document.id, b"document 2\n\ncontent\n").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document\n\ncontent 2\n").unwrap();
        lockbook_core::move_file(&db2, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/folder/document.md", b"document 2\n\ncontent 2\n")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_move_then_edit_content() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/folder/document.md", b"document content 2")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_edit_content_then_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/folder/document.md", b"document content 2")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_rename_then_edit_content() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2.md").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/document2.md", b"document content 2")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_edit_content_then_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::rename_file(&db, document.id, "document2.md").unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2.md"]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[("/document2.md", b"document content 2")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_then_edit_content() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, document.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_edit_content_then_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document.md"))
                .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, document.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_parent_then_edit_content() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/parent/document.md"),
        )
        .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_edit_content_then_delete_parent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let parent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/parent/")).unwrap();
        let document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/parent/document.md"),
        )
        .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, parent.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_delete_grandparent_then_edit_content() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document.md"),
        )
        .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn concurrent_change_edit_content_then_delete_grandparent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let grandparent =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/grandparent/"))
                .unwrap();
        let _parent = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/"),
        )
        .unwrap();
        let document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/grandparent/parent/document.md"),
        )
        .unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, grandparent.id).unwrap();
        lockbook_core::write_document(&db2, document.id, b"document content 2").unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests which are constructed to test cycle resolution
        Like those above, these are tests that setup two synced devices, operate on both devices, then sync both twice
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c/", "/c/b/", "/c/b/a/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/", "/c/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/d/", "/d/c/", "/d/c/b/", "/d/c/b/a/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c/", "/c/b/", "/c/b/a/", "/d/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/", "/d/", "/d/c/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/", "/c/", "/d/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_renames_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_renames_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_renames_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_renames_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_renames_first_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_renames_first_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_renames_first_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_renames_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_renames_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_renames_second_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_renames_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db2, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_renames_second_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db2, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_renames_second_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db2, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_renames_second_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::rename_file(&db2, a.id, "a2").unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        lockbook_core::rename_file(&db2, c.id, "c2").unwrap();
        lockbook_core::rename_file(&db2, d.id, "d2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_deletes_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_deletes_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_deletes_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_deletes_first_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_deletes_first_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_deletes_first_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_deletes_first_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_deletes_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_deletes_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_deletes_second_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_deletes_second_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_deletes_second_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_deletes_second_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_deletes_second_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_deletes_moving_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_deletes_moving_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_deletes_moving_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_deletes_moving_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_deletes_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_deletes_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_deletes_moving_device()
    {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_deletes_non_moving_device() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db2, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_deletes_non_moving_device(
    ) {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::delete_file(&db, b.id).unwrap();
        lockbook_core::delete_file(&db, c.id).unwrap();
        lockbook_core::delete_file(&db, d.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_two_cycle_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/b/", "/b/a/", "/b/child/", "/b/a/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_one_move_reverted_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/c/", "/c/b/", "/c/b/a/", "/c/child/", "/c/b/child/", "/c/b/a/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_three_cycle_two_moves_reverted_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/b/", "/b/a/", "/c/", "/b/child/", "/b/a/child/", "/c/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_one_move_reverted_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();
        let _d_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &[
                "/",
                "/d/",
                "/d/c/",
                "/d/c/b/",
                "/d/c/b/a/",
                "/d/child/",
                "/d/c/child/",
                "/d/c/b/child/",
                "/d/c/b/a/child/",
            ],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_adjacent_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();
        let _d_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &[
                "/",
                "/c/",
                "/c/b/",
                "/c/b/a/",
                "/d/",
                "/c/child/",
                "/c/b/child/",
                "/c/b/a/child/",
                "/d/child/",
            ],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_two_moves_reverted_alternating_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();
        let _d_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &[
                "/",
                "/b/",
                "/b/a/",
                "/d/",
                "/d/c/",
                "/b/child/",
                "/b/a/child/",
                "/d/child/",
                "/d/c/child/",
            ],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn cycle_resolution_concurrent_move_four_cycle_three_moves_reverted_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/")).unwrap();
        let c = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/")).unwrap();
        let d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/child/")).unwrap();
        let _b_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/b/child/")).unwrap();
        let _c_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/c/child/")).unwrap();
        let _d_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/d/child/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, b.id).unwrap();
        lockbook_core::move_file(&db2, b.id, c.id).unwrap();
        lockbook_core::move_file(&db2, c.id, d.id).unwrap();
        lockbook_core::move_file(&db2, d.id, a.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &[
                "/",
                "/b/",
                "/b/a/",
                "/c/",
                "/d/",
                "/b/child/",
                "/b/a/child/",
                "/c/child/",
                "/d/child/",
            ],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests which are constructed to test path conflict resolution
        Like those above, these are tests that setup two synced devices, operate on both devices, then sync both twice
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn path_conflict_resolution_concurrent_create_documents() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a.md", "/a-1.md"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b""), ("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folders() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a.md/", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folders_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a_child =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_document_then_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a.md", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folder_then_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a-1.md", "/a.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_document_then_folder_with_child() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a.md", "/a-1.md/", "/a-1.md/child/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folder_with_child_then_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        let _a =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a.md")).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a-1.md", "/a.md/", "/a.md/child/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_then_create_documents() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md", "/a-1.md"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b""), ("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_then_move_documents() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md", "/a-1.md"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b""), ("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_then_create_folders() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_then_move_folders() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md/", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_then_create_folders_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();
        let _a_child = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/folder/a.md/child/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_then_move_folders_with_children() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();
        let _a_child = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/folder/a.md/child/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a.md/", "/a-1.md/", "/a.md/child/", "/a-1.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_document_then_create_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folder_then_move_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a-1.md", "/a.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_folder_then_create_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a-1.md", "/a.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_document_then_move_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/a.md", "/a-1.md/"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_document_then_create_folder_with_child() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a.md", "/a-1.md/", "/a-1.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_folder_with_child_then_move_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md"))
            .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/")).unwrap();
        let _a2_child =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md/child/"))
                .unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a-1.md", "/a.md/", "/a.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_move_folder_with_child_then_create_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();
        let _a_child = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/folder/a.md/child/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a-1.md", "/a.md/", "/a.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn path_conflict_resolution_concurrent_create_document_then_move_folder_with_child() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/a.md/"))
            .unwrap();
        let _a_child = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, "/folder/a.md/child/"),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::move_file(&db, a.id, root.id).unwrap();
        let _a2 =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a.md")).unwrap();

        lockbook_core::sync_all(&db2, None).unwrap(); // note: order reversed
        lockbook_core::sync_all(&db, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(
            &db,
            &root,
            &["/", "/folder/", "/a.md", "/a-1.md/", "/a-1.md/child/"],
        );
        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Uncategorized tests
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn test_path_conflict() {
        let db1 = test_utils::test_config();

        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/new.md")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/new.md")).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        assert_eq!(
            lockbook_core::list_metadatas(&db2)
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
        let db1 = test_utils::test_config();

        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/new-1.md")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/new-1.md")).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        assert_eq!(
            lockbook_core::list_metadatas(&db2)
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
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1 = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file1.md"))
            .unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file1.id,
            )
            .unwrap(),
        )
        .unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file1.md")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
    }

    #[test]
    fn fuzzer_stuck_test() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b")).unwrap();
        let c = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/c/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/c/d/")).unwrap();
        lockbook_core::move_file(&db1, b.id, d.id).unwrap();
        lockbook_core::move_file(&db1, c.id, d.id).unwrap_err();
    }

    #[test]
    fn fuzzer_get_updates_required_test() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let document =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/document"))
                .unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::write_document(&db1, document.id, b"document content").unwrap();
        lockbook_core::write_document(&db2, document.id, b"content document").unwrap();
        lockbook_core::delete_file(&db2, document.id).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
    }
}
