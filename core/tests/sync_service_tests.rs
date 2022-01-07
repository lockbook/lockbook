mod integration_test;

#[cfg(test)]
mod sync_tests {
    use itertools::Itertools;

    use lockbook_core::assert_dirty_ids; // todo: remove
    use lockbook_core::model::repo::RepoSource;
    use lockbook_core::pure_functions::files;
    use lockbook_core::repo::metadata_repo; // todo: remove?
    use lockbook_core::service::{file_service, path_service, sync_service, test_utils};

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that operate on one device without syncing
     *  ------------------------------------------------------------------------------------------------------------ */

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

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that operate on one device and sync
     *  (work should be none)
     *  ------------------------------------------------------------------------------------------------------------ */

    #[test]
    fn synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        test_utils::sync(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/"]);
        test_utils::assert_all_document_contents(&db, &root, &[]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        test_utils::sync(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        test_utils::sync(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    #[test]
    fn synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        test_utils::sync(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
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

        test_utils::sync(&db);

        test_utils::assert_repo_integrity(&db);
        test_utils::assert_all_paths(&db, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db, &[]);
        test_utils::assert_server_work_ids(&db, &[]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that operate on one device, sync it, then create a new device without syncing
     *  (new device should have no files, local work should be empty, server work should include root)
     *  ------------------------------------------------------------------------------------------------------------ */

    #[test]
    fn new_unsynced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        test_utils::sync(&db);
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

        test_utils::sync(&db);
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

        test_utils::sync(&db);
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(
            &db2,
            &[root.id, a.id, b.id, c.id, d.id],
        );
    }

    #[test]
    fn new_unsynced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        test_utils::sync(&db);
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

        test_utils::sync(&db);
        let db2 = test_utils::make_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &[]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[root.id, folder.id, document.id]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that operate on one device, sync it, then create and sync a new device
     *  (work should be none, devices dbs should be equal)
     *  ------------------------------------------------------------------------------------------------------------ */

    #[test]
    fn new_synced_device_unmodified() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);
        test_utils::sync(&db);
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/"]);
        test_utils::assert_all_document_contents(&db2, &root, &[]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
    }

    #[test]
    fn new_synced_device_new_file() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();

        test_utils::sync(&db);
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
    }

    #[test]
    fn new_synced_device_new_files() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let _d =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/a/b/c/d")).unwrap();

        test_utils::sync(&db);
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/a/", "/a/b/", "/a/b/c/", "/a/b/c/d"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/a/b/c/d", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
    }

    #[test]
    fn new_synced_device_edited_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let document =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/document")).unwrap();
        lockbook_core::write_document(&db, document.id, b"document content").unwrap();

        test_utils::sync(&db);
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_dbs_eq(&db, &db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/document"]);
        test_utils::assert_all_document_contents(&db, &root, &[("/document", b"document content")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
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

        test_utils::sync(&db);
        let db2 = test_utils::make_and_sync_new_client(&db);

        test_utils::assert_repo_integrity(&db2);
        test_utils::assert_all_paths(&db2, &root, &["/", "/folder/", "/folder/document"]);
        test_utils::assert_all_document_contents(&db2, &root, &[("/folder/document", b"")]);
        test_utils::assert_local_work_ids(&db2, &[]);
        test_utils::assert_server_work_ids(&db2, &[]);
    }

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that setup two synced devices, operate on one device, and sync it without syncing the other device
     *  ------------------------------------------------------------------------------------------------------------ */

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Tests that setup two synced devices, operate on one device, and sync both
     *  (work should be none, devices dbs should be equal)
     *  ------------------------------------------------------------------------------------------------------------ */

    /*  ---------------------------------------------------------------------------------------------------------------
     *  Uncategorized tests
     *  ------------------------------------------------------------------------------------------------------------ */

    #[test]
    fn test_move_document_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder1/test.txt"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "nice document".as_bytes())
            .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);
        test_utils::assert_dbs_eq(&db1, &db2);

        let new_folder =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder2/"))
                .unwrap();

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert_dirty_ids!(db1, 2);

        test_utils::sync(&db1);
        assert_dirty_ids!(db1, 0);
        assert_dirty_ids!(db2, 2);

        test_utils::sync(&db2);
        assert_dirty_ids!(db2, 0);

        assert_eq!(
            file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
            file_service::get_all_metadata(&db2, RepoSource::Local).unwrap()
        );

        assert_eq!(
            file_service::get_document(&db2, RepoSource::Local, &file).unwrap(),
            "nice document".as_bytes()
        );

        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_move_reject() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder1/test.txt"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "Wow, what a doc".as_bytes())
            .unwrap();

        let new_folder1 =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder2/"))
                .unwrap();

        let new_folder2 =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder3/"))
                .unwrap();

        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                new_folder1.id,
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db2);

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                new_folder2.id,
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);

        test_utils::assert_dbs_eq(&db1, &db2);

        assert_eq!(
            file_service::get_metadata(&db1, RepoSource::Local, file.id)
                .unwrap()
                .parent,
            new_folder1.id
        );
        assert_eq!(
            file_service::get_document(&db2, RepoSource::Local, &file).unwrap(),
            "Wow, what a doc".as_bytes()
        );
    }

    #[test]
    fn test_rename_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder1/test.txt"))
                .unwrap();

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_rename(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.parent,
                "folder1-new",
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        let file_from_path =
            path_service::get_by_path(&db2, &test_utils::path(&root, "/folder1-new")).unwrap();

        assert_eq!(file_from_path.decrypted_name, "folder1-new");
        assert_eq!(
            path_service::get_by_path(&db2, &test_utils::path(&root, "/folder1-new/"),)
                .unwrap()
                .decrypted_name,
            file_from_path.decrypted_name
        );
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_rename_reject_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder1/test.txt"))
                .unwrap();
        test_utils::sync(&db1);

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_rename(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.parent,
                "folder1-new",
            )
            .unwrap(),
        )
        .unwrap();

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_rename(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.parent,
                "folder2-new",
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db2);
        test_utils::sync(&db1);

        assert_eq!(
            &path_service::get_by_path(&db2, &test_utils::path(&root, "/folder2-new"),)
                .unwrap()
                .decrypted_name,
            "folder2-new"
        );
        assert_eq!(
            &path_service::get_by_path(&db2, &test_utils::path(&root, "/folder2-new/"),)
                .unwrap()
                .decrypted_name,
            "folder2-new"
        );
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn move_then_edit() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/test.txt"))
            .unwrap();
        test_utils::sync(&db1);

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_rename(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                "new_name.txt",
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);

        file_service::insert_document(&db1, RepoSource::Local, &file, "noice".as_bytes()).unwrap();
        test_utils::sync(&db1);
    }

    // #[test]
    // fn sync_fs_invalid_state_via_rename() {
    //     let db1 = test_utils::test_config();
    //     let (_account, root) = test_utils::create_account(&db1);

    //     let file1 =
    //         lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/test.txt")).unwrap();
    //     let file2 =
    //         lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/test2.txt")).unwrap();
    //     test_utils::sync(&db1);

    //     test_utils::make_and_sync_new_clie&2, db1);
    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &files::apply_rename(
    //             &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
    //             file1.id,
    //             "test3.txt",
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     test_utils::sync(&db2);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &files::apply_rename(
    //             &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //             file2.id,
    //             "test3.txt",
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     // Just operate on the server work
    //     sync_service::calculate_work(&db1)
    //         .unwrap()
    //         .work_units
    //         .into_iter()
    //         .filter(|work| match work {
    //             WorkUnit::LocalChange { .. } => false,
    //             WorkUnit::ServerChange { .. } => true,
    //         })
    //         .for_each(|work| sync_service::execute_work(&db1, &account, work).unwrap());

    //     assert!(integrity_service::test_repo_integrity(&db1).is_ok());

    //     test_utils::assert_n_work_units(&db1, 1);

    //     test_utils::sync(&db1);
    //     test_utils::sync(&db2);

    //     assert_eq!(
    //         file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //         file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
    //     );

    //     test_utils::assert_dbs_eq(&db1, &db2);
    // }

    // #[test]
    // fn sync_fs_invalid_state_via_move() {
    //     let db1 = test_utils::test_config();
    //     let (_account, root) = test_utils::create_account(&db1);

    //     let file1 = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/test.txt"))
    //         .unwrap();
    //     let file2 = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/test.txt"))
    //         .unwrap();

    //     test_utils::sync(&db1);

    //     test_utils::make_and_sync_new_clie&2, db1);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &files::apply_move(
    //             &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //             file1.id,
    //             root_repo::get(&db1).unwrap(),
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     test_utils::sync(&db1);

    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &files::apply_move(
    //             &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
    //             file2.id,
    //             root_repo::get(&db2).unwrap(),
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();

    //     sync_service::calculate_work(&db2)
    //         .unwrap()
    //         .work_units
    //         .into_iter()
    //         .filter(|work| match work {
    //             WorkUnit::LocalChange { .. } => false,
    //             WorkUnit::ServerChange { .. } => true,
    //         })
    //         .for_each(|work| sync_service::execute_work(&db2, &account, work).unwrap());

    //     integrity_service::test_repo_integrity(&db2).unwrap();

    //     test_utils::assert_n_work_units(&db1, 0);
    //     test_utils::assert_n_work_units(&db2, 1);

    //     test_utils::sync(&db2);
    //     test_utils::sync(&db1);

    //     assert_eq!(
    //         file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //         file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
    //     );

    //     test_utils::assert_dbs_eq(&db1, &db2);
    // }

    #[test]
    fn test_content_conflict_unmergable() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/test.bin"))
            .unwrap();

        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "some good content".as_bytes(),
        )
        .unwrap();

        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);
        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "some new content".as_bytes(),
        )
        .unwrap();
        test_utils::sync(&db1);

        file_service::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "some offline content".as_bytes(),
        )
        .unwrap();
        let works = sync_service::calculate_work(&db2).unwrap();

        assert_eq!(works.work_units.len(), 2);

        test_utils::sync(&db2);

        let all_metadata = file_service::get_all_metadata(&db2, RepoSource::Base).unwrap();
        assert!(all_metadata
            .into_iter()
            .any(|m| m.decrypted_name.contains("test-1.bin")));

        test_utils::sync(&db1);

        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_path_conflict() {
        let db1 = test_utils::test_config();

        let (_account, root) = test_utils::create_account(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/new.md")).unwrap();
        test_utils::sync(&db1);
        lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/new.md")).unwrap();
        test_utils::sync(&db2);

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
        test_utils::sync(&db1);
        lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/new-1.md")).unwrap();
        test_utils::sync(&db2);

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
    fn test_content_conflict_mergable() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/mergable_file.md"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes())
            .unwrap();

        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        test_utils::sync(&db1);
        file_service::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        test_utils::sync(&db2);
        test_utils::sync(&db1);

        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Offline Line"));
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_move_before_mergable() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/mergable_file.md"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes())
            .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        test_utils::sync(&db1);
        let folder =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/folder1/"))
                .unwrap();
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_service::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        test_utils::sync(&db2);
        test_utils::sync(&db1);

        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Offline Line"));
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_after_before_mergable() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/mergable_file.md"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes())
            .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        test_utils::sync(&db1);
        let folder =
            lockbook_core::create_file_at_path(&db2, &test_utils::path(&root, "/folder1/"))
                .unwrap();
        file_service::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        test_utils::sync(&db2);
        test_utils::sync(&db1);

        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Offline Line"));
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_server_after_before_mergable() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/mergable_file.md"))
                .unwrap();

        file_service::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes())
            .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        let folder =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/folder1/"))
                .unwrap();
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);
        file_service::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        test_utils::sync(&db2);
        test_utils::sync(&db1);

        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_service::get_document(&db1, RepoSource::Local, &file).unwrap()
        )
        .contains("Offline Line"));
        test_utils::assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_not_really_editing_should_not_cause_work() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let file =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/file.md")).unwrap();

        file_service::insert_document(&db, RepoSource::Local, &file, "original".as_bytes())
            .unwrap();
        test_utils::sync(&db);
        assert_dirty_ids!(db, 0);

        file_service::insert_document(&db, RepoSource::Local, &file, "original".as_bytes())
            .unwrap();
        assert_dirty_ids!(db, 0);
    }

    #[test]
    fn test_not_really_renaming_should_not_cause_work() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let file =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/file.md")).unwrap();

        test_utils::sync(&db);
        assert_dirty_ids!(db, 0);
        lockbook_core::rename_file(&db, file.id, "file.md").unwrap();
        assert_dirty_ids!(db, 0);
    }

    #[test]
    fn test_not_really_moving_should_not_cause_work() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);

        let file =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/file.md")).unwrap();

        test_utils::sync(&db);
        assert_dirty_ids!(db, 0);

        file_service::insert_metadatum(
            &db,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db, RepoSource::Local).unwrap(),
                file.id,
                file.parent,
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    // Test that documents are deleted when a fresh sync happens
    fn delete_document_test_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file.md")).unwrap();

        test_utils::sync(&db1);
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            file_service::get_metadata(&db1, RepoSource::Local, file.id)
                .unwrap()
                .deleted
        );
        test_utils::sync(&db1);
        assert!(metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        let db2 = test_utils::make_new_client(&db1);
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());
        test_utils::sync(&db2);
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        assert!(file_service::get_document(&db2, RepoSource::Local, &file).is_err());
    }

    #[test]
    fn delete_new_document_never_synced() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file.md")).unwrap();

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        assert_dirty_ids!(db1, 1);

        assert!(
            metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_document(&db1, RepoSource::Local, &file)
                .unwrap()
                .is_some()
        );
        assert!(file_service::get_document(&db1, RepoSource::Local, &file).is_ok());
    }

    #[test]
    // Test that documents are deleted after a sync
    fn delete_document_test_after_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file.md")).unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);
        test_utils::sync(&db2);

        assert!(metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
            .unwrap()
            .is_none());
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        assert!(
            file_service::maybe_get_document(&db1, RepoSource::Local, &file)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_document(&db2, RepoSource::Local, &file)
                .unwrap()
                .is_none()
        );

        assert_eq!(
            file_service::get_all_metadata_changes(&db1).unwrap().len(),
            0
        );
        assert_eq!(
            file_service::get_all_metadata_changes(&db2).unwrap().len(),
            0
        );
    }

    #[test]
    fn test_folder_deletion() {
        // Create 3 files in a folder that is going to be deleted and 3 in a folder that won't
        // Sync 2 dbs
        // Delete them in the second db
        // Only 1 instruction should be in the work
        // Sync this from db2
        // 4 instructions should be in work for db1
        // Sync it
        // Make sure all the contents for those 4 files are gone from both dbs
        // Make sure all the contents for the stay files are there in both dbs

        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file1.md"))
                .unwrap();
        let file2_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file2.md"))
                .unwrap();
        let file3_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file3.md"))
                .unwrap();

        let file1_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file1.md"))
                .unwrap();
        let file2_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file2.md"))
                .unwrap();
        let file3_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file3.md"))
                .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);
        let to_delete =
            path_service::get_by_path(&db2, &test_utils::path(&root, "/delete")).unwrap();
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_dirty_ids!(db2, 1);
        test_utils::sync(&db2);

        // deleted files and their descendents are purged after sync
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );

        assert_dirty_ids!(db1, 4);
        test_utils::sync(&db1);

        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn test_moving_a_document_out_of_a_folder_before_delete_sync() {
        // Create 3 files in a folder that is going to be deleted and 3 in a folder that won't
        // Sync 2 dbs
        // Move a doc out
        // Delete them in the second db
        // Only 1 instruction should be in the work
        // Sync this from db2
        // 4 instructions should be in work for db1
        // Sync it
        // Make sure all the contents for those 4 files are gone from both dbs
        // Make sure all the contents for the stay files are there in both dbs

        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file1.md"))
                .unwrap();
        let file2_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file2A.md"))
                .unwrap();
        let file3_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/delete/file3.md"))
                .unwrap();

        let file1_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file1.md"))
                .unwrap();
        let file2_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file2.md"))
                .unwrap();
        let file3_stay =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/stay/file3.md"))
                .unwrap();
        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file2_delete.id,
                file1_stay.parent,
            )
            .unwrap(),
        )
        .unwrap();
        let to_delete =
            path_service::get_by_path(&db2, &test_utils::path(&root, "/delete")).unwrap();
        file_service::insert_metadatum(
            &db2,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder + moved document should show up as the sync instructions
        assert_dirty_ids!(db2, 2);
        test_utils::sync(&db2);

        // deleted files and their ancestors purged after sync
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );

        assert_dirty_ids!(db1, 4);
        test_utils::sync(&db1);

        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn create_new_folder_and_move_old_files_into_it_then_delete_that_folder() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/old/file1.md"))
                .unwrap();
        let file2_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/old/file2.md"))
                .unwrap();
        let file3_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/old/file3.md"))
                .unwrap();
        let file4_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/old/file4.md"))
                .unwrap();

        test_utils::sync(&db1);

        let new_folder =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/new/")).unwrap();
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file2_delete.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_move(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file4_delete.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_service::maybe_get_metadata(&db1, RepoSource::Local, file4_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_metadata(&db1, RepoSource::Local, new_folder.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        test_utils::sync(&db1);

        let db2 = test_utils::make_and_sync_new_client(&db1);

        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_service::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, file4_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_service::maybe_get_metadata(&db2, RepoSource::Local, new_folder.id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn create_document_sync_delete_document_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1 = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file1.md"))
            .unwrap();

        test_utils::sync(&db1);
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
        test_utils::sync(&db1);
        assert_dirty_ids!(db1, 0);
    }

    #[test]
    fn deleted_path_is_released() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file1 = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file1.md"))
            .unwrap();
        test_utils::sync(&db1);

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
        test_utils::sync(&db1);

        lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/file1.md")).unwrap();
        test_utils::sync(&db1);
    }

    #[test]
    fn folder_delete_bug() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        lockbook_core::create_file_at_path(
            &db1,
            &test_utils::path(&root, "/test/folder/document.md"),
        )
        .unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        let folder_to_delete =
            path_service::get_by_path(&db1, &test_utils::path(&root, "/test")).unwrap();
        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                folder_to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();
        test_utils::sync(&db1);

        test_utils::sync(&db2); // There was an error here
    }

    #[test]
    fn ensure_that_deleting_a_file_doesnt_make_it_show_up_in_work_calculated() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);

        let file = lockbook_core::create_file_at_path(
            &db1,
            &test_utils::path(&root, "/test/folder/document.md"),
        )
        .unwrap();
        test_utils::sync(&db1);

        file_service::insert_metadatum(
            &db1,
            RepoSource::Local,
            &files::apply_delete(
                &file_service::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert_dirty_ids!(db1, 1);

        test_utils::sync(&db1);

        assert_dirty_ids!(db1, 0);
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

        test_utils::sync(&db1);
        test_utils::sync(&db2);

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
        test_utils::sync(&db1);

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
        test_utils::sync(&db2);
    }

    // #[test]
    // fn recreate_smail_bug_attempt_3() {
    //     let db1 = test_utils::test_config();
    //     let (_account, root) = test_utils::create_account(&db1);

    //     let parent = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/tmp/")).unwrap();
    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &files::create(Folder, parent.id, "child", &account.username),
    //     )
    //     .unwrap();

    //     test_utils::sync(&db1);

    //     test_utils::make_and_sync_new_clie&2, db1);

    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &files::create(Folder, parent.id, "child2", &account.username),
    //     )
    //     .unwrap();
    //     let work = sync_service::calculate_work(&db2).unwrap().work_units; // 1 piece of work, the new child
    //     test_utils::assert_n_work_units(&db2, 1);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &files::apply_delete(
    //             &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //             parent.id,
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     test_utils::sync(&db1);

    //     for wu in work {
    //         sync_service::execute_work(&db2, &account, wu).unwrap_err();
    //     }

    //     // Uninstall and fresh sync
    //     let db3 = test_utils::test_config();
    //     account_service::import_account(&db3, &account_service::export_account(&db1).unwrap())
    //         .unwrap();

    //     sync_service::sync(&db3, None).unwrap();
    //     test_utils::assert_no_metadata_problems(&db3);
    // }

    #[test]
    fn issue_734_bug() {
        let db1 = test_utils::test_config();
        let (account, root) = test_utils::create_account(&db1);

        lockbook_core::create_file_at_path(
            &db1,
            &test_utils::path(&root, &format!("/{}", account.username)),
        )
        .unwrap();

        test_utils::sync(&db1);
    }

    #[test]
    fn cycle_in_a_sync() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::move_file(&db2, a.id, b.id).unwrap();

        test_utils::sync(&db1);
        test_utils::sync(&db2);
    }

    #[test]
    fn cycle_in_a_sync_with_a_rename() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::rename_file(&db1, a.id, "new_name").unwrap();
        test_utils::sync(&db1);

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        test_utils::sync(&db2);
    }

    #[test]
    fn cycle_in_a_sync_with_a_delete() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db1, b.id).unwrap();
        test_utils::sync(&db1);

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        lockbook_core::delete_file(&db2, b.id).unwrap();
        test_utils::sync(&db2);
    }

    #[test]
    fn cycle_in_a_sync_with_a_delete_2() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let a = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/")).unwrap();
        let b = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/b/")).unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, b.id, a.id).unwrap();
        lockbook_core::delete_file(&db1, a.id).unwrap();
        test_utils::sync(&db1);

        lockbook_core::move_file(&db2, a.id, b.id).unwrap();
        lockbook_core::delete_file(&db2, a.id).unwrap();
        test_utils::sync(&db2);
    }

    #[test]
    fn delete_folder_with_document() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);
        test_utils::sync(&db);
        let f = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/f/")).unwrap();
        let _d = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/f/d")).unwrap();
        lockbook_core::delete_file(&db, f.id).unwrap();
        for _ in 0..2 {
            test_utils::sync(&db);
        }
        test_utils::assert_repo_integrity(&db);
        assert!(lockbook_core::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());
    }

    #[test]
    fn delete_folder_with_folder() {
        let db = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db);
        test_utils::sync(&db);
        let f1 = lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/f/")).unwrap();
        let _f2 =
            lockbook_core::create_file_at_path(&db, &test_utils::path(&root, "/f/f2/")).unwrap();
        lockbook_core::delete_file(&db, f1.id).unwrap();
        for _ in 0..2 {
            test_utils::sync(&db);
        }
        test_utils::assert_repo_integrity(&db);
        assert!(lockbook_core::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());
    }

    #[test]
    fn move_into_new_folder_and_delete_folder() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let to_move =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/to_move/"))
                .unwrap();
        let to_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/to_delete/"))
                .unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db1, to_move.id, to_delete.id).unwrap();
        lockbook_core::delete_file(&db1, to_delete.id).unwrap();
        test_utils::sync(&db1);

        test_utils::sync(&db2);
    }

    #[test]
    fn move_into_new_folder_and_delete_folders_parent() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let to_move =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/to_move/"))
                .unwrap();
        let to_delete =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/to_delete/"))
                .unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        let intermediate_folder = lockbook_core::create_file_at_path(
            &db1,
            &test_utils::path(&root, "/to_delete/intermediate_folder/"),
        )
        .unwrap();
        lockbook_core::move_file(&db1, to_move.id, intermediate_folder.id).unwrap();
        lockbook_core::delete_file(&db1, to_delete.id).unwrap();
        test_utils::sync(&db1);

        test_utils::sync(&db2);
    }

    #[test]
    fn new_file_in_folder_deleted_by_other_client() {
        let db1 = test_utils::test_config();
        let (_account, root) = test_utils::create_account(&db1);
        let b =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/b/")).unwrap();
        let d =
            lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/c/d/")).unwrap();
        let e = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/a/c/d/e/"))
            .unwrap();
        test_utils::sync(&db1);
        let db2 = test_utils::make_and_sync_new_client(&db1);

        lockbook_core::move_file(&db2, b.id, e.id).unwrap();
        lockbook_core::delete_file(&db1, d.id).unwrap();
        lockbook_core::delete_file(&db2, d.id).unwrap();
        let _f = lockbook_core::create_file_at_path(&db1, &test_utils::path(&root, "/f")).unwrap();

        for _ in 0..2 {
            test_utils::sync(&db1);
            test_utils::sync(&db2);
        }
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
