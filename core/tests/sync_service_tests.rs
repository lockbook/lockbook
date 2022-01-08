mod integration_test;

#[cfg(test)]
mod sync_tests {
    use itertools::Itertools;

    use lockbook_core::model::repo::RepoSource;
    use lockbook_core::pure_functions::files;
    use lockbook_core::service::{file_service, path_service, test_utils};

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device without syncing
    ---------------------------------------------------------------------------------------------------------------- */

    #[test]
    fn unsynced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/")).unwrap();
        let c =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db, &[a.id, b.id, c.id, d.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[folder.id, document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::delete_file(&db, document.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[document.id]); // todo: deleting an unsynced document should not result in local work
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_device_folder_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();
        lockbook_core::delete_file(&db, folder.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[folder.id, document.id]); // todo: deleting an unsynced folder should not result in local work for the folder or its contents
        test_utils::assert_server_work_ids(&db, &[]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device and sync
        (work should be none, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------- */

    #[test]
    fn synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_new_file_same_name_as_username() {
        let db = test_utils::test_config();
        let (account, root) = test_utils::create_account(&db);

        let _document = lockbook_core::create_file_at_path(
            &db,
            &test_utils::path(&root, &format!("/{}", account.username)),
        )
        .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", &format!("/{}", account.username)]);
        test_utils::assert_all_document_contents(
            &db,
            &root,
            &[(&format!("/{}", account.username), b"")],
        );
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_rename() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::rename_file(&db, document.id, "document2").unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document2"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document2", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_delete() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::delete_file(&db, document.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    #[test]
    fn synced_device_delete_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();
        lockbook_core::delete_file(&db, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
        test_utils::assert_deleted_files_pruned(&db);
        test_utils::assert_new_synced_client_dbs_eq(&db);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device after syncing
    ---------------------------------------------------------------------------------------------------------------- */

    #[test]
    fn unsynced_change_synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        lockbook_core::sync_all(&db, None).unwrap();

        let a = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/")).unwrap();
        let c =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db, &[a.id, b.id, c.id, d.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_edit_unedit() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::write_document(&db, document.id, b"document content").unwrap();
        lockbook_core::write_document(&db, document.id, b"").unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn unsynced_change_synced_device_move() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::move_file(&db, document.id, folder.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[document.id]);
        test_utils::assert_server_work_ids(&db, &[]);
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
    fn unsynced_change_synced_device_delete_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        lockbook_core::delete_file(&db, folder.id).unwrap();

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[folder.id]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that operate on one device, sync it, then create a new device without syncing
        (new device should have no files, local work should be empty, server work should include root)
    ---------------------------------------------------------------------------------------------------------------- */

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
    fn new_unsynced_device_delete_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();
        lockbook_core::delete_file(&db, folder.id).unwrap();

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
    ---------------------------------------------------------------------------------------------------------------- */

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
    fn new_synced_device_delete_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();
        lockbook_core::delete_file(&db, folder.id).unwrap();

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
    ---------------------------------------------------------------------------------------------------------------- */

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
    fn unsynced_change_new_synced_device_delete_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let folder =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/")).unwrap();
        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/folder/document"))
                .unwrap();

        lockbook_core::sync_all(&db, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db);

        lockbook_core::delete_file(&db, folder.id).unwrap();

        lockbook_core::sync_all(&db, None).unwrap();

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[folder.id, document.id]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
        Tests that setup two synced devices, operate on one device, and sync both
        (work should be none, devices dbs should be equal, deleted files should be pruned)
    ---------------------------------------------------------------------------------------------------------------- */

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
    ---------------------------------------------------------------------------------------------------------------- */

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
            &[
                "/",
                "/parent2/",
                "/parent2/parent/",
                "/parent2/parent/document",
            ],
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
            &[
                "/",
                "/parent2/",
                "/parent2/parent/",
                "/parent2/parent/document",
            ],
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

    /*  ---------------------------------------------------------------------------------------------------------------
        Uncategorized tests
    ---------------------------------------------------------------------------------------------------------------- */

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

    // Create two sets of folders `temp/1-100md` on two clients.
    // Sync both
    // One will become `temp-RENAME-CONFLICT` or something like that
    // You have to delete off one client `temp` while the other tries to process a server change that it no longer has.
    // (not the problem)
    #[test]
    fn recreate_smail_bug() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        for i in 1..10 {
            lockbook_core::create_file_at_path(
                &db1,
                &test_utils::path(&root, &format!("/tmp/{}/", i)),
            )
            .unwrap();
        }

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();

        let file_to_break =
            path_service::get_by_path(&db1, &test_utils::path(&root, "/tmp")).unwrap();

        // 1 Client renames and syncs
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_rename(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file_to_break.id,
                "tmp2",
            )
            .unwrap(),
        )
        .unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        // Other deletes and syncs
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file_to_break.id,
            )
            .unwrap(),
        )
        .unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
    }

    #[test]
    fn cycle_in_a_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::move_file(&db2, a.id, b.id).unwrap();

        lockbook_core::sync_all(&db1, None).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
    }

    #[test]
    fn cycle_in_a_sync_with_a_rename() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::rename_file(&db1, a.id, "new_name").unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
    }

    #[test]
    fn cycle_in_a_sync_with_a_delete() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db1, b.id).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
    }

    #[test]
    fn cycle_in_a_sync_with_a_delete_2() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db1, a.id).unwrap();
        lockbook_core::sync_all(&db1, None).unwrap();

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        lockbook_core::sync_all(&db2, None).unwrap();
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
}
