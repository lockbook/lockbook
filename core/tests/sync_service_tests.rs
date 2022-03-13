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
                Operation::Move { client_num: 0, path: "/folder/document", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, root, &[]);
                    },
                },
            ],
            // rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
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
                        test_utils::assert_local_work_paths(&db, root, &["/document2"]);
                    },
                },
            ],
            // rename_unrename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Rename { client_num: 0, path: "/document2", new_name: "document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_local_work_paths(&db, root, &[]);
                    },
                },
            ],
            // delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, root, &["/document"]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, root, &["/parent/"]);
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_local_work_paths(&db, root, &["/grandparent/"]);
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

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device, sync it, then create a new device without syncing
        (new device should have no files, local work should be empty, server work should include root)
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn new_unsynced_device() {
        for mut ops in [
            // unmodified
            vec![
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/"]);
                    },
                },
            ],
            // new_file
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/", "/document"]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(
                            &db,
                            &root,
                            &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                    },
                },
            ],
            // edited_document
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/", "/document"]);
                    },
                },
            ],
            // move
            vec![
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/folder/document"],
                        );
                    },
                },
            ],
            // rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/", "/document2"]);
                    },
                },
            ],
            // delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/"]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/"]);
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Sync { client_num: 0 },
                Operation::Client { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_server_work_paths(&db, &root, &["/"]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[1].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_all_paths(&db, &root, &[]);
                    test_utils::assert_all_document_contents(&db, &root, &[]);
                    test_utils::assert_local_work_paths(&db, root, &[]);
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
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // new_file
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[0].1;
                    let db2 = &dbs[1].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_dbs_eq(&db, &db2);
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
        Tests that setup two synced devices, operate on one device, and sync it without syncing the other device
    ---------------------------------------------------------------------------------------------------------------  */

    #[test]
    fn unsynced_change_new_synced_device() {
        for mut ops in [
            // unmodified
            vec![
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                    },
                },
            ],
            // new_file
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &["/document"]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                        test_utils::assert_server_work_paths(
                            &db,
                            &root,
                            &["/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"],
                        );
                    },
                },
            ],
            // edited_document
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_server_work_paths(&db, &root, &["/document"]);
                    },
                },
            ],
            // move
            vec![
                Operation::Create { client_num: 0, path: "/folder/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_server_work_paths(&db, &root, &["/document"]);
                    },
                },
            ],
            // rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_server_work_paths(&db, &root, &["/document"]);
                    },
                },
            ],
            // delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                        test_utils::assert_server_work_paths(&db, &root, &["/document"]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document", b"")],
                        );
                        test_utils::assert_server_work_paths(
                            &db,
                            &root,
                            &["/parent/", "/parent/document"],
                        );
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Sync { client_num: 0 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &[
                                "/",
                                "/grandparent/",
                                "/grandparent/parent/",
                                "/grandparent/parent/document",
                            ],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/grandparent/parent/document", b"")],
                        );
                        test_utils::assert_server_work_paths(
                            &db,
                            &root,
                            &[
                                "/grandparent/",
                                "/grandparent/parent/",
                                "/grandparent/parent/document",
                            ],
                        );
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[1].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_local_work_paths(&db, &root, &[]);
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
                Operation::Sync { client_num: 1 },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // new_file
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                    },
                },
            ],
            // new_files
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a/b/c/d" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit { client_num: 0, path: "/document", content: b"document content" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/folder/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
        ] {
            ops.push(Operation::Custom {
                f: &|dbs, root| {
                    let db = &dbs[0].1;
                    let db2 = &dbs[1].1;
                    test_utils::assert_repo_integrity(&db);
                    test_utils::assert_dbs_eq(&db, &db2);
                    test_utils::assert_local_work_paths(&db, &root, &[]);
                    test_utils::assert_server_work_paths(&db, &root, &[]);
                    test_utils::assert_deleted_files_pruned(&db);
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
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/parent/" },
                Operation::Move { client_num: 1, path: "/document", new_parent_path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document", b"")],
                        );
                    },
                },
            ],
            // different_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/parent2/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/parent/" },
                Operation::Move { client_num: 1, path: "/document", new_parent_path: "/parent2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent2/", "/parent/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document", b"")],
                        );
                    },
                },
            ],
            // identical_rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Rename { client_num: 1, path: "/document", new_name: "document2" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2", b"")],
                        );
                    },
                },
            ],
            // different_rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Rename { client_num: 1, path: "/document", new_name: "document3" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2", b"")],
                        );
                    },
                },
            ],
            // move_then_rename
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/parent/" },
                Operation::Rename { client_num: 1, path: "/document", new_name: "document2" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document2"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document2", b"")],
                        );
                    },
                },
            ],
            // rename_then_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Move { client_num: 1, path: "/document", new_parent_path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document2"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document2", b"")],
                        );
                    },
                },
            ],
            // identical_delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Delete { client_num: 1, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // identical_delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent_then_direct
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Delete { client_num: 1, path: "/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_direct_then_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/document" },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // identical_delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Delete { client_num: 1, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent_then_direct
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Delete { client_num: 1, path: "/grandparent/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_direct_then_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Delete { client_num: 1, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent_then_parent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Delete { client_num: 1, path: "/grandparent/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent_then_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/parent/" },
                Operation::Delete { client_num: 1, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // move_then_delete
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/parent/" },
                Operation::Delete { client_num: 1, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/parent/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_then_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Move { client_num: 1, path: "/document", new_parent_path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/parent/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // move_then_delete_new_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/document", new_parent_path: "/parent/" },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_new_parent_then_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Move { client_num: 1, path: "/document", new_parent_path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // move_then_delete_old_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/parent/document", new_parent_path: "/" },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
                    },
                },
            ],
            // delete_old_parent_then_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Move { client_num: 1, path: "/parent/document", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // rename_then_delete
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document", new_name: "document2" },
                Operation::Delete { client_num: 1, path: "/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_then_rename
            vec![
                Operation::Create { client_num: 0, path: "/document" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document" },
                Operation::Rename { client_num: 1, path: "/document", new_name: "document2" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // create_then_move_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/parent2/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Move { client_num: 1, path: "/parent/", new_parent_path: "/parent2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent2/parent/document", b"")],
                        );
                    },
                },
            ],
            // move_parent_then_create
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/parent2/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/parent/", new_parent_path: "/parent2/" },
                Operation::Create { client_num: 1, path: "/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent2/", "/parent2/parent/", "/parent2/parent/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent2/parent/document", b"")],
                        );
                    },
                },
            ],
            // create_then_rename_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Rename { client_num: 1, path: "/parent/", new_name: "parent2" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent2/", "/parent2/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent2/document", b"")],
                        );
                    },
                },
            ],
            // rename_parent_then_create
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/parent/", new_name: "parent2" },
                Operation::Create { client_num: 1, path: "/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent2/", "/parent2/document"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent2/document", b"")],
                        );
                    },
                },
            ],
            // create_then_delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/parent/document" },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent_then_create
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 1, path: "/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // create_then_delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/grandparent/parent/document" },
                Operation::Delete { client_num: 1, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent_then_create
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Create { client_num: 1, path: "/grandparent/parent/document" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // identical_content_edit_not_mergable
            vec![
                Operation::Create { client_num: 0, path: "/document.draw" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.draw",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.draw",
                    content: b"document content 2",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.draw",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/document.draw", "/document-1.draw"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[
                                ("/document.draw", b"document content 2"),
                                ("/document-1.draw", b"document content 2"),
                            ],
                        );
                    },
                },
            ],
            // identical_content_edit_mergable
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document.md"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document.md", b"document content 2")],
                        );
                    },
                },
            ],
            // different_content_edit_not_mergable
            vec![
                Operation::Create { client_num: 0, path: "/document.draw" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.draw",
                    content: b"document\n\ncontent\n",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.draw",
                    content: b"document 2\n\ncontent\n",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.draw",
                    content: b"document\n\ncontent 2\n",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/document.draw", "/document-1.draw"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[
                                ("/document.draw", b"document 2\n\ncontent\n"),
                                ("/document-1.draw", b"document\n\ncontent 2\n"),
                            ],
                        );
                    },
                },
            ],
            // different_content_edit_mergable
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document\n\ncontent\n",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document 2\n\ncontent\n",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document\n\ncontent 2\n",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document.md"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document.md", b"document 2\n\ncontent 2\n")],
                        );
                    },
                },
            ],
            // different_content_edit_mergable_with_move_in_first_sync
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document\n\ncontent\n",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document 2\n\ncontent\n",
                },
                Operation::Move {
                    client_num: 0,
                    path: "/document.md",
                    new_parent_path: "/parent/",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document\n\ncontent 2\n",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document.md", b"document 2\n\ncontent 2\n")],
                        );
                    },
                },
            ],
            // different_content_edit_mergable_with_move_in_second_sync
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document\n\ncontent\n",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document 2\n\ncontent\n",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document\n\ncontent 2\n",
                },
                Operation::Move {
                    client_num: 1,
                    path: "/document.md",
                    new_parent_path: "/parent/",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document.md", b"document 2\n\ncontent 2\n")],
                        );
                    },
                },
            ],
            // move_then_edit_content
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move {
                    client_num: 0,
                    path: "/document.md",
                    new_parent_path: "/parent/",
                },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document.md", b"document content 2")],
                        );
                    },
                },
            ],
            // edit_content_then_move
            vec![
                Operation::Create { client_num: 0, path: "/parent/" },
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Move {
                    client_num: 1,
                    path: "/document.md",
                    new_parent_path: "/parent/",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/parent/", "/parent/document.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/parent/document.md", b"document content 2")],
                        );
                    },
                },
            ],
            // rename_then_edit_content
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/document.md", new_name: "document2.md" },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2.md"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2.md", b"document content 2")],
                        );
                    },
                },
            ],
            // edit_content_then_rename
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Rename { client_num: 1, path: "/document.md", new_name: "document2.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/document2.md"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/document2.md", b"document content 2")],
                        );
                    },
                },
            ],
            // delete_then_edit_content
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 1,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // edit_content_then_delete
            vec![
                Operation::Create { client_num: 0, path: "/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/document.md",
                    content: b"document content 2",
                },
                Operation::Delete { client_num: 1, path: "/document.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_parent_then_edit_content
            vec![
                Operation::Create { client_num: 0, path: "/parent/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/parent/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/parent/" },
                Operation::Edit {
                    client_num: 1,
                    path: "/parent/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // edit_content_then_delete_parent
            vec![
                Operation::Create { client_num: 0, path: "/parent/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/parent/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/parent/document.md",
                    content: b"document content 2",
                },
                Operation::Delete { client_num: 1, path: "/parent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // delete_grandparent_then_edit_content
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/grandparent/parent/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Delete { client_num: 0, path: "/grandparent/" },
                Operation::Edit {
                    client_num: 1,
                    path: "/grandparent/parent/document.md",
                    content: b"document content 2",
                },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // edit_content_then_delete_grandparent
            vec![
                Operation::Create { client_num: 0, path: "/grandparent/parent/document.md" },
                Operation::Edit {
                    client_num: 0,
                    path: "/grandparent/parent/document.md",
                    content: b"document content",
                },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Edit {
                    client_num: 0,
                    path: "/grandparent/parent/document.md",
                    content: b"document content 2",
                },
                Operation::Delete { client_num: 1, path: "/grandparent/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
        ] {
            let checks = ops.pop().unwrap();
            ops.extend(vec![
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        let db2 = &dbs[1].1;
                        test_utils::assert_repo_integrity(&db);
                        test_utils::assert_dbs_eq(&db, &db2);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                        test_utils::assert_deleted_files_pruned(&db);
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
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/c/", "/c/b/", "/c/b/a/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/b/a/", "/c/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/d/", "/d/c/", "/d/c/b/", "/d/c/b/a/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/c/", "/c/b/", "/c/b/a/", "/d/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b/", "/b/a/", "/d/", "/d/c/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_three_moves_reverted
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b/", "/b/a/", "/c/", "/d/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // two_cycle_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 0, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 0, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 0, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 0, path: "/c2/", new_parent_path: "/d2/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 0, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 0, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 0, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c2/", new_parent_path: "/d2/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_three_moves_reverted_with_renames_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Rename { client_num: 0, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 0, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 0, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 0, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 0, path: "/a2/", new_parent_path: "/b2/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // two_cycle_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Move { client_num: 1, path: "/b2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Move { client_num: 1, path: "/c2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Move { client_num: 1, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 1, path: "/c2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b2/", "/b2/a2/", "/c2/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 1, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 1, path: "/d2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/d2/", "/d2/c2/", "/d2/c2/b2/", "/d2/c2/b2/a2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 1, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 1, path: "/c2/", new_parent_path: "/d2/" },
                Operation::Move { client_num: 1, path: "/d2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/c2/", "/c2/b2/", "/c2/b2/a2/", "/d2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 1, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 1, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b2/", "/b2/a2/", "/d2/", "/d2/c2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_three_moves_reverted_with_renames_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Rename { client_num: 1, path: "/a/", new_name: "a2" },
                Operation::Rename { client_num: 1, path: "/b/", new_name: "b2" },
                Operation::Rename { client_num: 1, path: "/c/", new_name: "c2" },
                Operation::Rename { client_num: 1, path: "/d/", new_name: "d2" },
                Operation::Move { client_num: 1, path: "/b2/", new_parent_path: "/c2/" },
                Operation::Move { client_num: 1, path: "/c2/", new_parent_path: "/d2/" },
                Operation::Move { client_num: 1, path: "/d2/", new_parent_path: "/a2/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b2/", "/b2/a2/", "/c2/", "/d2/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // two_cycle_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/b/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/c/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/b/" },
                Operation::Delete { client_num: 0, path: "/c/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/d/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/c/" },
                Operation::Delete { client_num: 0, path: "/d/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/b/" },
                Operation::Delete { client_num: 0, path: "/d/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_three_moves_reverted_with_deletes_first_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 0, path: "/b/" },
                Operation::Delete { client_num: 0, path: "/c/" },
                Operation::Delete { client_num: 0, path: "/d/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // two_cycle_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Delete { client_num: 1, path: "/b/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/c/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/c/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Delete { client_num: 1, path: "/b/" },
                Operation::Delete { client_num: 1, path: "/c/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/d/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Delete { client_num: 1, path: "/b/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/c/", "/d/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Delete { client_num: 1, path: "/c/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/d/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_three_moves_reverted_with_deletes_second_device
            vec![
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 0, path: "/b/" },
                Operation::Create { client_num: 0, path: "/c/" },
                Operation::Create { client_num: 0, path: "/d/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Delete { client_num: 1, path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/b/", "/c/", "/d/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // move_two_cycle_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b/", "/b/a/", "/b/child/", "/b/a/child/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_one_move_reverted_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &[
                                "/",
                                "/c/",
                                "/c/b/",
                                "/c/b/a/",
                                "/c/child/",
                                "/c/b/child/",
                                "/c/b/a/child/",
                            ],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // three_cycle_two_moves_reverted_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/b/", "/b/a/", "/c/", "/b/child/", "/b/a/child/", "/c/child/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // four_cycle_one_move_reverted_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Create { client_num: 0, path: "/d/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                    },
                },
            ],
            // four_cycle_two_moves_reverted_adjacent_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Create { client_num: 0, path: "/d/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 0, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                    },
                },
            ],
            // four_cycle_two_moves_reverted_alternating_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Create { client_num: 0, path: "/d/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 0, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                    },
                },
            ],
            // four_cycle_three_moves_reverted_with_children
            vec![
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 0, path: "/b/child/" },
                Operation::Create { client_num: 0, path: "/c/child/" },
                Operation::Create { client_num: 0, path: "/d/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/a/", new_parent_path: "/b/" },
                Operation::Move { client_num: 1, path: "/b/", new_parent_path: "/c/" },
                Operation::Move { client_num: 1, path: "/c/", new_parent_path: "/d/" },
                Operation::Move { client_num: 1, path: "/d/", new_parent_path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
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
                    },
                },
            ],
        ] {
            let checks = ops.pop().unwrap();
            ops.extend(vec![
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        let db2 = &dbs[1].1;
                        test_utils::assert_repo_integrity(&db);
                        test_utils::assert_dbs_eq(&db, &db2);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                        test_utils::assert_deleted_files_pruned(&db);
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
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md" },
                Operation::Create { client_num: 1, path: "/a.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/a.md", "/a-1.md"]);
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/a.md", b""), ("/a-1.md", b"")],
                        );
                    },
                },
            ],
            // concurrent_create_folders
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a/" },
                Operation::Create { client_num: 1, path: "/a/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/a/", "/a-1/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // concurrent_create_folders_with_children
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a/child/" },
                Operation::Create { client_num: 1, path: "/a/child/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a/", "/a-1/", "/a/child/", "/a-1/child/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // concurrent_create_document_then_folder
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md" },
                Operation::Create { client_num: 1, path: "/a.md/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/a.md", "/a-1.md/"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
                    },
                },
            ],
            // concurrent_create_folder_then_document
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md/" },
                Operation::Create { client_num: 1, path: "/a.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(&db, &root, &["/", "/a.md/", "/a-1.md"]);
                        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
                    },
                },
            ],
            // concurrent_create_document_then_folder_with_child
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md" },
                Operation::Create { client_num: 1, path: "/a.md/child/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a.md", "/a-1.md/", "/a-1.md/child/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[("/a.md", b"")]);
                    },
                },
            ],
            // concurrent_create_folder_with_child_then_document
            vec![
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md/child/" },
                Operation::Create { client_num: 1, path: "/a.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/a.md/", "/a.md/child/", "/a-1.md"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[("/a-1.md", b"")]);
                    },
                },
            ],
            // concurrent_move_then_create_documents
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/folder/a.md", new_parent_path: "/" },
                Operation::Create { client_num: 1, path: "/a.md" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/a.md", "/a-1.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/a.md", b""), ("/a-1.md", b"")],
                        );
                    },
                },
            ],
            // concurrent_create_then_move_documents
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md" },
                Operation::Move { client_num: 1, path: "/folder/a.md", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/a.md", "/a-1.md"],
                        );
                        test_utils::assert_all_document_contents(
                            &db,
                            &root,
                            &[("/a.md", b""), ("/a-1.md", b"")],
                        );
                    },
                },
            ],
            // concurrent_move_then_create_folders
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/folder/a.md/", new_parent_path: "/" },
                Operation::Create { client_num: 1, path: "/a.md/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/a.md/", "/a-1.md/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // concurrent_create_then_move_folders
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md/" },
                Operation::Move { client_num: 1, path: "/folder/a.md/", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &["/", "/folder/", "/a.md/", "/a-1.md/"],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // concurrent_move_then_create_folders_with_children
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Move { client_num: 0, path: "/folder/a.md/", new_parent_path: "/" },
                Operation::Create { client_num: 1, path: "/a.md/child/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &[
                                "/",
                                "/folder/",
                                "/a.md/",
                                "/a-1.md/",
                                "/a.md/child/",
                                "/a-1.md/child/",
                            ],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
            // concurrent_create_then_move_folders_with_children
            vec![
                Operation::Create { client_num: 0, path: "/folder/a.md/child/" },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Create { client_num: 0, path: "/a.md/child/" },
                Operation::Move { client_num: 1, path: "/folder/a.md/", new_parent_path: "/" },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[1].1;
                        test_utils::assert_all_paths(
                            &db,
                            &root,
                            &[
                                "/",
                                "/folder/",
                                "/a.md/",
                                "/a-1.md/",
                                "/a.md/child/",
                                "/a-1.md/child/",
                            ],
                        );
                        test_utils::assert_all_document_contents(&db, &root, &[]);
                    },
                },
            ],
        ] {
            let checks = ops.pop().unwrap();
            ops.extend(vec![
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Sync { client_num: 0 },
                Operation::Sync { client_num: 1 },
                Operation::Custom {
                    f: &|dbs, root| {
                        let db = &dbs[0].1;
                        let db2 = &dbs[1].1;
                        test_utils::assert_repo_integrity(&db);
                        test_utils::assert_dbs_eq(&db, &db2);
                        test_utils::assert_local_work_paths(&db, &root, &[]);
                        test_utils::assert_server_work_paths(&db, &root, &[]);
                        test_utils::assert_deleted_files_pruned(&db);
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
    fn fuzzer_stuck_test_2() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        let a = lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a/")).unwrap();
        let b =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a/b/")).unwrap();
        lockbook_core::move_file(&db2, b.id, root.id).unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        let _c = lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/c/")).unwrap();
        lockbook_core::move_file(&db2, b.id, a.id).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn fuzzer_stuck_test_3() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        let _a = lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a/")).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);

        let _b =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/b.md")).unwrap();
        let c = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/c")).unwrap();
        lockbook_core::rename_file(&db1, c.id, "c2").unwrap();

        let _d =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/d")).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn fuzzer_stuck_test_4() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        let _a = lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a/")).unwrap();
        let b =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/a/b/")).unwrap();
        lockbook_core::move_file(&db2, b.id, root.id).unwrap();
        lockbook_core::rename_file(&db2, b.id, "b2").unwrap();
        let c =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/c.md")).unwrap();
        lockbook_core::write_document(&db2, c.id, b"DPCN8G0CK8qXSyJhervmmEXFnkt").unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    // this is the one that actually stuck the fuzzer (1 through 4 did not)
    #[test]
    fn fuzzer_stuck_test_5() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/b/")).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);

        lockbook_core::move_file(&db1, b.id, root.id).unwrap();
        lockbook_core::move_file(&db1, a.id, b.id).unwrap();
        lockbook_core::delete_file(&db1, b.id).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
        test_utils::assert_repo_integrity(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);
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
