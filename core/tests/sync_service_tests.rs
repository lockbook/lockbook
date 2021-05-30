mod integration_test;

#[cfg(test)]
mod sync_tests {
    use lockbook_core::repo::{document_repo, file_metadata_repo, local_changes_repo};
    use lockbook_core::service::test_utils::{assert_dbs_eq, generate_account, test_config};
    use lockbook_core::service::{account_service, file_service, sync_service};
    use lockbook_models::file_metadata::FileType::Folder;
    use lockbook_models::work_unit::WorkUnit;

    macro_rules! assert_no_metadata_problems (
        ($db:expr) => {
            assert!(file_metadata_repo::test_repo_integrity($db)
                .unwrap()
                .is_empty());
        }
    );

    macro_rules! assert_n_work_units {
        ($db:expr, $n:literal) => {
            assert_eq!(
                sync_service::calculate_work(&$db).unwrap().work_units.len(),
                $n
            );
        };
    }

    macro_rules! make_account {
        ($db:expr) => {{
            let generated_account = generate_account();
            let account = account_service::create_account(
                &$db,
                &generated_account.username,
                &generated_account.api_url,
            )
            .unwrap();
            account
        }};
    }

    macro_rules! path {
        ($account:expr, $path:expr) => {{
            &format!("{}/{}", $account.username, $path)
        }};
    }

    macro_rules! make_new_client {
        ($new_client:ident, $old_client:expr) => {
            let $new_client = test_config();
            account_service::import_account(
                &$new_client,
                &account_service::export_account(&$old_client).unwrap(),
            )
            .unwrap();
        };
    }

    macro_rules! make_and_sync_new_client {
        ($new_client:ident, $old_client:expr) => {
            make_new_client!($new_client, $old_client);
            sync!(&$new_client);
        };
    }

    #[macro_export]
    macro_rules! sync {
        ($config:expr, $f:expr) => {
            sync_service::sync($config, $f).unwrap()
        };
        ($config:expr) => {
            sync_service::sync($config, None).unwrap()
        };
    }

    #[test]
    fn test_create_files_and_folders_sync() {
        let db = test_config();
        let account = make_account!(db);

        assert_n_work_units!(db, 0);

        file_service::create_at_path(&db, &format!("{}/a/b/c/test", account.username)).unwrap();
        assert_n_work_units!(db, 4);

        sync!(&db);

        make_new_client!(db2, db);
        assert_n_work_units!(db2, 5);

        sync!(&db2);
        assert_eq!(
            file_metadata_repo::get_all(&db).unwrap(),
            file_metadata_repo::get_all(&db2).unwrap()
        );
        assert_n_work_units!(db2, 0);
    }

    #[test]
    fn test_edit_document_sync() {
        let db = &test_config();
        let account = make_account!(db);

        assert_n_work_units!(db, 0);
        println!("1st calculate work");

        let file =
            file_service::create_at_path(&db, &format!("{}/a/b/c/test", account.username)).unwrap();

        sync!(&db);
        println!("1st sync done");

        make_and_sync_new_client!(db2, db);
        println!("2nd sync done, db2");

        file_service::write_document(&db, file.id, "meaningful messages".as_bytes()).unwrap();

        assert_n_work_units!(db, 1);
        println!("2nd calculate work, db1, 1 dirty file");

        match sync_service::calculate_work(&db)
            .unwrap()
            .work_units
            .get(0)
            .unwrap()
            .clone()
        {
            WorkUnit::LocalChange { metadata } => assert_eq!(metadata.name, file.name),
            WorkUnit::ServerChange { .. } => {
                panic!("This should have been a local change with no server changes!")
            }
        };
        println!("3rd calculate work, db1, 1 dirty file");

        sync!(&db);
        println!("3rd sync done, db1, dirty file pushed");

        assert_n_work_units!(db, 0);
        println!("4th calculate work, db1, dirty file pushed");

        assert_n_work_units!(db2, 1);
        println!("5th calculate work, db2, dirty file needs to be pulled");

        let edited_file = file_metadata_repo::get(&db, file.id).unwrap();

        match sync_service::calculate_work(&db2)
            .unwrap()
            .work_units
            .get(0)
            .unwrap()
            .clone()
        {
            WorkUnit::ServerChange { metadata } => assert_eq!(metadata, edited_file),
            WorkUnit::LocalChange { .. } => {
                panic!("This should have been a ServerChange with no LocalChange!")
            }
        };
        println!("6th calculate work, db2, dirty file needs to be pulled");

        sync!(&db2);
        println!("4th sync done, db2, dirty file pulled");

        assert_n_work_units!(db2, 0);
        println!("7th calculate work ");

        assert_eq!(
            file_service::read_document(&db2, edited_file.id).unwrap(),
            "meaningful messages".as_bytes()
        );
        assert_dbs_eq(&db, &db2);
    }

    #[test]
    fn test_move_document_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "nice document".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        assert_dbs_eq(&db1, &db2);

        let new_folder =
            file_service::create_at_path(&db1, &format!("{}/folder2/", account.username)).unwrap();

        file_service::move_file(&db1, file.id, new_folder.id).unwrap();
        assert_n_work_units!(db1, 2);

        sync!(&db1);
        assert_n_work_units!(db1, 0);
        assert_n_work_units!(db2, 2);

        sync!(&db2);
        assert_n_work_units!(db2, 0);

        assert_eq!(
            file_metadata_repo::get_all(&db1).unwrap(),
            file_metadata_repo::get_all(&db2).unwrap()
        );

        assert_eq!(
            file_service::read_document(&db2, file.id).unwrap(),
            "nice document".as_bytes()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_move_reject() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "Wow, what a doc".as_bytes()).unwrap();

        let new_folder1 =
            file_service::create_at_path(&db1, &format!("{}/folder2/", account.username)).unwrap();

        let new_folder2 =
            file_service::create_at_path(&db1, &format!("{}/folder3/", account.username)).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        file_service::move_file(&db2, file.id, new_folder1.id).unwrap();
        sync!(&db2);

        file_service::move_file(&db1, file.id, new_folder2.id).unwrap();
        sync!(&db1);

        assert_dbs_eq(&db1, &db2);

        assert_eq!(
            file_metadata_repo::get(&db1, file.id).unwrap().parent,
            new_folder1.id
        );
        assert_eq!(
            file_service::read_document(&db2, file.id).unwrap(),
            "Wow, what a doc".as_bytes()
        );
    }

    #[test]
    fn test_rename_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_service::rename_file(&db1, file.parent, "folder1-new").unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        assert_eq!(
            file_metadata_repo::get_by_path(&db2, &format!("{}/folder1-new", account.username),)
                .unwrap()
                .unwrap()
                .name,
            "folder1-new"
        );
        assert_eq!(
            file_metadata_repo::get_by_path(&db2, &format!("{}/folder1-new/", account.username),)
                .unwrap()
                .unwrap()
                .name,
            "folder1-new"
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_rename_reject_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();
        sync!(&db1);

        file_service::rename_file(&db1, file.parent, "folder1-new").unwrap();

        make_and_sync_new_client!(db2, db1);

        file_service::rename_file(&db2, file.parent, "folder2-new").unwrap();
        sync!(&db2);
        sync!(&db1);

        assert_eq!(
            file_metadata_repo::get_by_path(&db2, &format!("{}/folder2-new", account.username),)
                .unwrap()
                .unwrap()
                .name,
            "folder2-new"
        );
        assert_eq!(
            file_metadata_repo::get_by_path(&db2, &format!("{}/folder2-new/", account.username),)
                .unwrap()
                .unwrap()
                .name,
            "folder2-new"
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn move_then_edit() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/test.txt", account.username)).unwrap();
        sync!(&db1);

        file_service::rename_file(&db1, file.id, "new_name.txt").unwrap();
        sync!(&db1);

        file_service::write_document(&db1, file.id, "noice".as_bytes()).unwrap();
        sync!(&db1);
    }

    #[test]
    fn sync_fs_invalid_state_via_rename() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file1 =
            file_service::create_at_path(&db1, &format!("{}/test.txt", account.username)).unwrap();
        let file2 =
            file_service::create_at_path(&db1, &format!("{}/test2.txt", account.username)).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        file_service::rename_file(&db2, file1.id, "test3.txt").unwrap();
        sync!(&db2);

        file_service::rename_file(&db1, file2.id, "test3.txt").unwrap();
        // Just operate on the server work
        sync_service::calculate_work(&db1)
            .unwrap()
            .work_units
            .into_iter()
            .filter(|work| match work {
                WorkUnit::LocalChange { .. } => false,
                WorkUnit::ServerChange { .. } => true,
            })
            .for_each(|work| sync_service::execute_work(&db1, &account, work).unwrap());

        assert!(file_metadata_repo::test_repo_integrity(&db1)
            .unwrap()
            .is_empty());

        assert_n_work_units!(db1, 1);

        sync!(&db1);
        sync!(&db2);

        assert_eq!(
            file_metadata_repo::get_all(&db1).unwrap(),
            file_metadata_repo::get_all(&db2).unwrap()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn sync_fs_invalid_state_via_move() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file1 = file_service::create_at_path(&db1, &format!("{}/a/test.txt", account.username))
            .unwrap();
        let file2 = file_service::create_at_path(&db1, &format!("{}/b/test.txt", account.username))
            .unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::move_file(
            &db1,
            file1.id,
            file_metadata_repo::get_root(&db1).unwrap().unwrap().id,
        )
        .unwrap();
        sync!(&db1);

        file_service::move_file(
            &db2,
            file2.id,
            file_metadata_repo::get_root(&db2).unwrap().unwrap().id,
        )
        .unwrap();

        sync_service::calculate_work(&db2)
            .unwrap()
            .work_units
            .into_iter()
            .filter(|work| match work {
                WorkUnit::LocalChange { .. } => false,
                WorkUnit::ServerChange { .. } => true,
            })
            .for_each(|work| sync_service::execute_work(&db2, &account, work).unwrap());

        assert!(file_metadata_repo::test_repo_integrity(&db2)
            .unwrap()
            .is_empty());

        assert_n_work_units!(db1, 0);
        assert_n_work_units!(db2, 1);

        sync!(&db2);
        sync!(&db1);

        assert_eq!(
            file_metadata_repo::get_all(&db1).unwrap(),
            file_metadata_repo::get_all(&db2).unwrap()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_unmergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/test.bin", account.username)).unwrap();

        file_service::write_document(&db1, file.id, "some good content".as_bytes()).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        file_service::write_document(&db1, file.id, "some new content".as_bytes()).unwrap();
        sync!(&db1);

        file_service::write_document(&db2, file.id, "some offline content".as_bytes()).unwrap();
        let works = sync_service::calculate_work(&db2).unwrap();

        assert_eq!(works.work_units.len(), 2);

        for work in works.clone().work_units {
            sync_service::execute_work(&db2, &account, work).unwrap();
        }

        let works = sync_service::calculate_work(&db2).unwrap();
        assert_eq!(works.work_units.len(), 1);

        match works.work_units.get(0).unwrap() {
            WorkUnit::LocalChange { metadata } => {
                assert!(metadata.name.contains("CONTENT-CONFLICT"))
            }
            WorkUnit::ServerChange { .. } => panic!("This should not be the work type"),
        }

        sync!(&db2);
        sync!(&db1);

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        sync!(&db1);
        file_service::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes()).unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 1")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 2")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Offline Line")
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_move_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        sync!(&db1);
        let folder =
            file_service::create_at_path(&db2, &format!("{}/folder1/", account.username)).unwrap();
        file_service::move_file(&db2, file.id, folder.id).unwrap();
        file_service::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes()).unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 1")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 2")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Offline Line")
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_after_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        sync!(&db1);
        let folder =
            file_service::create_at_path(&db2, &format!("{}/folder1/", account.username)).unwrap();
        file_service::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes()).unwrap();
        file_service::move_file(&db2, file.id, folder.id).unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 1")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 2")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Offline Line")
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_server_after_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_service::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        let folder =
            file_service::create_at_path(&db1, &format!("{}/folder1/", account.username)).unwrap();
        file_service::move_file(&db1, file.id, folder.id).unwrap();
        sync!(&db1);
        file_service::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes()).unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 1")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Line 2")
        );
        assert!(
            String::from_utf8_lossy(&file_service::read_document(&db1, file.id).unwrap())
                .contains("Offline Line")
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_not_really_editing_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            file_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        file_service::write_document(&db, file.id, "original".as_bytes()).unwrap();
        sync!(&db);
        assert_n_work_units!(db, 0);

        file_service::write_document(&db, file.id, "original".as_bytes()).unwrap();
        assert_n_work_units!(db, 0);
    }

    #[test]
    fn test_not_really_renaming_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            file_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db);
        assert_n_work_units!(db, 0);

        assert!(file_service::rename_file(&db, file.id, "file.md").is_err());
        assert_n_work_units!(db, 0);
    }

    #[test]
    fn test_not_really_moving_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            file_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db);
        assert_n_work_units!(db, 0);

        assert!(file_service::move_file(&db, file.id, file.parent).is_err());
    }

    #[test]
    // Test that documents are deleted when a fresh sync happens
    fn delete_document_test_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db1);
        file_service::delete_document(&db1, file.id).unwrap();
        assert!(file_metadata_repo::get(&db1, file.id).unwrap().deleted);
        sync!(&db1);
        assert!(file_metadata_repo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());

        make_new_client!(db2, db1);
        assert!(file_metadata_repo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());
        sync!(&db2);
        assert!(file_metadata_repo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());

        assert!(file_service::read_document(&db2, file.id).is_err());
    }

    #[test]
    fn delete_new_document_never_synced() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();

        file_service::delete_document(&db1, file.id).unwrap();
        assert_n_work_units!(db1, 0);

        assert!(file_metadata_repo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(document_repo::maybe_get(&db1, file.id).unwrap().is_none());
        assert!(file_service::read_document(&db1, file.id).is_err());
    }

    #[test]
    // Test that documents are deleted after a sync
    fn delete_document_test_after_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::delete_document(&db1, file.id).unwrap();
        sync!(&db1);
        sync!(&db2);

        assert!(file_metadata_repo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());

        assert!(document_repo::maybe_get(&db1, file.id).unwrap().is_none());
        assert!(document_repo::maybe_get(&db2, file.id).unwrap().is_none());

        assert!(local_changes_repo::get_local_changes(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(local_changes_repo::get_local_changes(&db2, file.id)
            .unwrap()
            .is_none());
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

        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete = file_service::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete = file_service::create_at_path(&db1, &path("delete/file2.md")).unwrap();
        let file3_delete = file_service::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = file_service::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = file_service::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = file_service::create_at_path(&db1, &path("stay/file3.md")).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::delete_folder(
            &db2,
            file_metadata_repo::get_by_path(&db2, &path("delete"))
                .unwrap()
                .unwrap()
                .id,
        )
        .unwrap();

        assert!(
            file_metadata_repo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db2, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db2, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db2, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db2, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_n_work_units!(db2, 1);
        sync!(&db2);

        assert!(file_metadata_repo::maybe_get(&db2, file1_delete.parent)
            .unwrap()
            .is_none());

        assert_n_work_units!(db1, 4);
        sync!(&db1);

        assert!(file_metadata_repo::maybe_get(&db1, file1_delete.parent)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db1, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db1, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db1, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db1, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file3_stay.id)
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

        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete = file_service::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete = file_service::create_at_path(&db1, &path("delete/file2A.md")).unwrap();
        let file3_delete = file_service::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = file_service::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = file_service::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = file_service::create_at_path(&db1, &path("stay/file3.md")).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::move_file(&db2, file2_delete.id, file1_stay.parent).unwrap();
        file_service::delete_folder(
            &db2,
            file_metadata_repo::get_by_path(&db2, &path("delete"))
                .unwrap()
                .unwrap()
                .id,
        )
        .unwrap();

        assert!(
            file_metadata_repo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db2, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db2, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db2, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db2, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db2, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_n_work_units!(db2, 2);
        sync!(&db2);

        assert!(file_metadata_repo::maybe_get(&db2, file1_delete.parent)
            .unwrap()
            .is_none());

        assert_n_work_units!(db1, 4);
        sync!(&db1);

        assert!(file_metadata_repo::maybe_get(&db1, file1_delete.parent)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db1, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db1, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db1, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db1, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn create_new_folder_and_move_old_files_into_it_then_delete_that_folder() {
        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete = file_service::create_at_path(&db1, &path("old/file1.md")).unwrap();
        let file2_delete = file_service::create_at_path(&db1, &path("old/file2.md")).unwrap();
        let file3_delete = file_service::create_at_path(&db1, &path("old/file3.md")).unwrap();
        let file4_delete = file_service::create_at_path(&db1, &path("old/file4.md")).unwrap();

        sync!(&db1);

        let new_folder = file_service::create_at_path(&db1, &path("new/")).unwrap();
        file_service::move_file(&db1, file2_delete.id, new_folder.id).unwrap();
        file_service::move_file(&db1, file4_delete.id, new_folder.id).unwrap();
        file_service::delete_folder(&db1, new_folder.id).unwrap();

        assert!(
            !file_metadata_repo::maybe_get(&db1, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_metadata_repo::maybe_get(&db1, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_metadata_repo::maybe_get(&db1, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_metadata_repo::maybe_get(&db1, file4_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_metadata_repo::maybe_get(&db1, new_folder.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        assert!(
            !file_metadata_repo::maybe_get(&db2, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db2, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !file_metadata_repo::maybe_get(&db2, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(file_metadata_repo::maybe_get(&db2, file4_delete.id)
            .unwrap()
            .is_none());
        assert!(file_metadata_repo::maybe_get(&db2, new_folder.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn create_document_sync_delete_document_sync() {
        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = file_service::create_at_path(&db1, &path("file1.md")).unwrap();

        sync!(&db1);
        file_service::delete_document(&db1, file1.id).unwrap();
        sync!(&db1);
        assert_n_work_units!(db1, 0);
    }

    #[test]
    fn deleted_path_is_released() {
        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = file_service::create_at_path(&db1, &path("file1.md")).unwrap();
        sync!(&db1);

        file_service::delete_document(&db1, file1.id).unwrap();
        sync!(&db1);

        file_service::create_at_path(&db1, &path("file1.md")).unwrap();
        sync!(&db1);
    }

    #[test]
    fn folder_delete_bug() {
        let db1 = test_config();
        let account = make_account!(db1);

        file_service::create_at_path(&db1, path!(account, "test/folder/document.md")).unwrap();
        sync!(&db1);
        make_and_sync_new_client!(db2, db1);

        let folder_to_delete = file_metadata_repo::get_by_path(&db1, path!(account, "test"))
            .unwrap()
            .unwrap();
        file_service::delete_folder(&db1, folder_to_delete.id).unwrap();
        sync!(&db1);

        sync!(&db2); // There was an error here
    }

    #[test]
    fn ensure_that_deleting_a_file_doesnt_make_it_show_up_in_work_calculated() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            file_service::create_at_path(&db1, path!(account, "test/folder/document.md")).unwrap();
        sync!(&db1);

        file_service::delete_document(&db1, file.id).unwrap();

        let work = sync_service::calculate_work(&db1).unwrap();

        assert_n_work_units!(&db1, 1);

        sync_service::execute_work(&db1, &account, work.work_units[0].clone()).unwrap();

        assert_n_work_units!(&db1, 0);
    }

    // Create two sets of folders `temp/1-100md` on two clients.
    // Sync both
    // One will become `temp-RENAME-CONFLICT` or something like that
    // You have to delete off one client `temp` while the other tries to process a server change that it no longer has.
    // (not the problem)
    #[test]
    fn recreate_smail_bug() {
        let db1 = test_config();
        let account = make_account!(db1);

        make_and_sync_new_client!(db2, db1);

        for i in 1..100 {
            file_service::create_at_path(&db1, &format!("{}/tmp/{}/", account.username, i))
                .unwrap();
        }

        sync!(&db1);
        sync!(&db2);

        let file_to_break = file_metadata_repo::get_by_path(&db1, path!(account, "tmp"))
            .unwrap()
            .unwrap();

        // 1 Client renames and syncs
        file_service::rename_file(&db1, file_to_break.id, "tmp2").unwrap();
        sync!(&db1);

        // Other deletes and syncs
        file_service::delete_folder(&db2, file_to_break.id).unwrap();
        sync!(&db2);
    }

    #[test]
    fn recreate_smail_bug_attempt_3() {
        let db1 = test_config();
        let account = make_account!(db1);

        let parent = file_service::create_at_path(&db1, path!(account, "tmp/")).unwrap();
        file_service::create(&db1, "child", parent.id, Folder).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_service::create(&db2, "child2", parent.id, Folder).unwrap();
        let work = sync_service::calculate_work(&db2).unwrap().work_units; // 1 piece of work, the new child
        assert_n_work_units!(db2, 1);

        file_service::delete_folder(&db1, parent.id).unwrap();
        sync!(&db1);

        for wu in work {
            sync_service::execute_work(&db2, &account, wu).unwrap_err();
        }

        // Uninstall and fresh sync
        let db3 = test_config();
        account_service::import_account(&db3, &account_service::export_account(&db1).unwrap())
            .unwrap();

        sync_service::sync(&db3, None).unwrap();
        assert_no_metadata_problems!(&db3);
    }

    #[test]
    fn issue_734_bug() {
        let db1 = test_config();
        let account = make_account!(db1);

        file_service::create_at_path(&db1, &format!("{}/{}/", account.username, account.username))
            .unwrap();

        sync!(&db1);
    }
}
