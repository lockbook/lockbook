mod integration_test;

#[cfg(test)]
mod sync_tests {
    use itertools::Itertools;
    use lockbook_core::model::repo::RepoSource;
    use lockbook_core::repo::{file_repo, metadata_repo};
    use lockbook_core::service::test_utils::{assert_dbs_eq, generate_account, test_config};
    use lockbook_core::service::{account_service, file_service, path_service, sync_service};
    use lockbook_core::{path, rename_file};
    use lockbook_models::work_unit::WorkUnit;

    macro_rules! assert_dirty_ids {
        ($db:expr, $n:literal) => {
            assert_eq!(
                sync_service::calculate_work(&$db)
                    .unwrap()
                    .work_units
                    .into_iter()
                    .map(|wu| wu.get_metadata().id)
                    .unique()
                    .count(),
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
    fn test_even_more_basic_sync() {
        let db = test_config();
        let account = make_account!(db);

        println!("{}", line!());
        path_service::create_at_path(&db, path!(account, "test.md")).unwrap();
        assert_dirty_ids!(db, 1);
    }

    #[test]
    fn test_basic_sync() {
        let db = test_config();
        let account = make_account!(db);

        assert_dirty_ids!(db, 0);

        path_service::create_at_path(&db, path!(account, "test.md")).unwrap();
        assert_dirty_ids!(db, 1);
        sync!(&db);
        assert_dirty_ids!(db, 0);
    }

    #[test]
    fn test_create_files_and_folders_sync() {
        let db = test_config();
        let account = make_account!(db);

        assert_dirty_ids!(db, 0);

        path_service::create_at_path(&db, &format!("{}/a/b/c/test", account.username)).unwrap();
        assert_dirty_ids!(db, 4);

        sync!(&db);

        make_new_client!(db2, db);
        assert_dirty_ids!(db2, 5);

        sync!(&db2);
        assert_eq!(
            file_repo::get_all_metadata(&db, RepoSource::Local).unwrap(),
            file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
        );
        assert_dirty_ids!(db2, 0);
    }

    #[test]
    fn test_edit_document_sync() {
        let db = &test_config();
        let account = make_account!(db);

        assert_dirty_ids!(db, 0);
        println!("1st calculate work");

        let file =
            path_service::create_at_path(&db, &format!("{}/a/b/c/test", account.username)).unwrap();

        sync!(&db);
        println!("1st sync done");

        make_and_sync_new_client!(db2, db);
        println!("2nd sync done, db2");

        file_repo::insert_document(
            &db,
            RepoSource::Local,
            &file,
            "meaningful messages".as_bytes(),
        )
        .unwrap();

        assert_dirty_ids!(db, 1);
        println!("2nd calculate work, db1, 1 dirty file");

        match sync_service::calculate_work(&db)
            .unwrap()
            .work_units
            .get(0)
            .unwrap()
            .clone()
        {
            WorkUnit::LocalChange { metadata } => {
                assert_eq!(metadata.decrypted_name, file.decrypted_name)
            }
            WorkUnit::ServerChange { .. } => {
                panic!("This should have been a local change with no server changes!")
            }
        };
        println!("3rd calculate work, db1, 1 dirty file");

        sync!(&db);
        println!("3rd sync done, db1, dirty file pushed");

        assert_dirty_ids!(db, 0);
        println!("4th calculate work, db1, dirty file pushed");

        assert_dirty_ids!(db2, 1);
        println!("5th calculate work, db2, dirty file needs to be pulled");

        let edited_file = file_repo::get_metadata(&db, RepoSource::Local, file.id).unwrap();

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

        assert_dirty_ids!(db2, 0);
        println!("7th calculate work ");

        assert_eq!(
            file_repo::get_document(&db2, RepoSource::Local, edited_file.id).unwrap(),
            "meaningful messages".as_bytes()
        );
        assert_dbs_eq(&db, &db2);
    }

    #[test]
    fn test_move_document_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "nice document".as_bytes())
            .unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        assert_dbs_eq(&db1, &db2);

        let new_folder =
            path_service::create_at_path(&db1, &format!("{}/folder2/", account.username)).unwrap();

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert_dirty_ids!(db1, 2);

        sync!(&db1);
        assert_dirty_ids!(db1, 0);
        assert_dirty_ids!(db2, 2);

        sync!(&db2);
        assert_dirty_ids!(db2, 0);

        assert_eq!(
            file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
            file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
        );

        assert_eq!(
            file_repo::get_document(&db2, RepoSource::Local, file.id).unwrap(),
            "nice document".as_bytes()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_move_reject() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "Wow, what a doc".as_bytes())
            .unwrap();

        let new_folder1 =
            path_service::create_at_path(&db1, &format!("{}/folder2/", account.username)).unwrap();

        let new_folder2 =
            path_service::create_at_path(&db1, &format!("{}/folder3/", account.username)).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                new_folder1.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db2);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                new_folder2.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        assert_dbs_eq(&db1, &db2);

        assert_eq!(
            file_repo::get_metadata(&db1, RepoSource::Local, file.id)
                .unwrap()
                .parent,
            new_folder1.id
        );
        assert_eq!(
            file_repo::get_document(&db2, RepoSource::Local, file.id).unwrap(),
            "Wow, what a doc".as_bytes()
        );
    }

    #[test]
    fn test_rename_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_rename(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.parent,
                "folder1-new",
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        let file_from_path =
            path_service::get_by_path(&db2, &format!("{}/folder1-new", account.username)).unwrap();

        assert_eq!(file_from_path.decrypted_name, "folder1-new");
        assert_eq!(
            path_service::get_by_path(&db2, &format!("{}/folder1-new/", account.username),)
                .unwrap()
                .decrypted_name,
            file_from_path.decrypted_name
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_rename_reject_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/folder1/test.txt", account.username))
                .unwrap();
        sync!(&db1);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_rename(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.parent,
                "folder1-new",
            )
            .unwrap(),
        )
        .unwrap();

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_rename(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.parent,
                "folder2-new",
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db2);
        sync!(&db1);

        assert_eq!(
            &path_service::get_by_path(&db2, &format!("{}/folder2-new", account.username),)
                .unwrap()
                .decrypted_name,
            "folder2-new"
        );
        assert_eq!(
            &path_service::get_by_path(&db2, &format!("{}/folder2-new/", account.username),)
                .unwrap()
                .decrypted_name,
            "folder2-new"
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn move_then_edit() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/test.txt", account.username)).unwrap();
        sync!(&db1);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_rename(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                "new_name.txt",
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        file_repo::insert_document(&db1, RepoSource::Local, &file, "noice".as_bytes()).unwrap();
        sync!(&db1);
    }

    // #[test]
    // fn sync_fs_invalid_state_via_rename() {
    //     let db1 = test_config();
    //     let account = make_account!(db1);

    //     let file1 =
    //         path_service::create_at_path(&db1, &format!("{}/test.txt", account.username)).unwrap();
    //     let file2 =
    //         path_service::create_at_path(&db1, &format!("{}/test2.txt", account.username)).unwrap();
    //     sync!(&db1);

    //     make_and_sync_new_client!(db2, db1);
    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &file_service::apply_rename(
    //             &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
    //             file1.id,
    //             "test3.txt",
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     sync!(&db2);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &file_service::apply_rename(
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

    //     assert_n_work_units!(db1, 1);

    //     sync!(&db1);
    //     sync!(&db2);

    //     assert_eq!(
    //         file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //         file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
    //     );

    //     assert_dbs_eq(&db1, &db2);
    // }

    // #[test]
    // fn sync_fs_invalid_state_via_move() {
    //     let db1 = test_config();
    //     let account = make_account!(db1);

    //     let file1 = path_service::create_at_path(&db1, &format!("{}/a/test.txt", account.username))
    //         .unwrap();
    //     let file2 = path_service::create_at_path(&db1, &format!("{}/b/test.txt", account.username))
    //         .unwrap();

    //     sync!(&db1);

    //     make_and_sync_new_client!(db2, db1);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &file_service::apply_move(
    //             &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //             file1.id,
    //             root_repo::get(&db1).unwrap(),
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     sync!(&db1);

    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &file_service::apply_move(
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

    //     assert_n_work_units!(db1, 0);
    //     assert_n_work_units!(db2, 1);

    //     sync!(&db2);
    //     sync!(&db1);

    //     assert_eq!(
    //         file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //         file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap()
    //     );

    //     assert_dbs_eq(&db1, &db2);
    // }

    #[test]
    fn test_content_conflict_unmergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/test.bin", account.username)).unwrap();

        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "some good content".as_bytes(),
        )
        .unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "some new content".as_bytes(),
        )
        .unwrap();
        sync!(&db1);

        file_repo::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "some offline content".as_bytes(),
        )
        .unwrap();
        let works = sync_service::calculate_work(&db2).unwrap();

        assert_eq!(works.work_units.len(), 2);

        sync!(&db2);

        let all_metadata = file_repo::get_all_metadata(&db2, RepoSource::Base).unwrap();
        assert!(all_metadata.into_iter().any(|m| m.decrypted_name.contains("CONTENT-CONFLICT")));

        sync!(&db1);

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes()).unwrap();

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        sync!(&db1);
        file_repo::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_move_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        sync!(&db1);
        let folder =
            path_service::create_at_path(&db2, &format!("{}/folder1/", account.username)).unwrap();
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_repo::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_after_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        sync!(&db1);
        let folder =
            path_service::create_at_path(&db2, &format!("{}/folder1/", account.username)).unwrap();
        file_repo::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_server_after_before_mergable() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/mergable_file.md", account.username))
                .unwrap();

        file_repo::insert_document(&db1, RepoSource::Local, &file, "Line 1\n".as_bytes()).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_document(
            &db1,
            RepoSource::Local,
            &file,
            "Line 1\nLine 2\n".as_bytes(),
        )
        .unwrap();
        let folder =
            path_service::create_at_path(&db1, &format!("{}/folder1/", account.username)).unwrap();
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
                folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);
        file_repo::insert_document(
            &db2,
            RepoSource::Local,
            &file,
            "Line 1\nOffline Line\n".as_bytes(),
        )
        .unwrap();

        sync!(&db2);
        sync!(&db1);

        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &file_repo::get_document(&db1, RepoSource::Local, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_not_really_editing_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            path_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        file_repo::insert_document(&db, RepoSource::Local, &file, "original".as_bytes()).unwrap();
        sync!(&db);
        assert_dirty_ids!(db, 0);

        file_repo::insert_document(&db, RepoSource::Local, &file, "original".as_bytes()).unwrap();
        assert_dirty_ids!(db, 0);
    }

    #[test]
    fn test_not_really_renaming_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            path_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db);
        assert_dirty_ids!(db, 0);
        rename_file(&db, file.id, "file.md").unwrap();
        assert_dirty_ids!(db, 0);
    }

    #[test]
    fn test_not_really_moving_should_not_cause_work() {
        let db = test_config();
        let account = make_account!(db);

        let file =
            path_service::create_at_path(&db, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db);
        assert_dirty_ids!(db, 0);

        file_repo::insert_metadatum(
            &db,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db, RepoSource::Local).unwrap(),
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
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();

        sync!(&db1);
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            file_repo::get_metadata(&db1, RepoSource::Local, file.id)
                .unwrap()
                .deleted
        );
        sync!(&db1);
        assert!(metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        make_new_client!(db2, db1);
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());
        sync!(&db2);
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        assert!(file_repo::get_document(&db2, RepoSource::Local, file.id).is_err());
    }

    #[test]
    fn delete_new_document_never_synced() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        assert_dirty_ids!(db1, 1);

        assert!(metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
            .unwrap().unwrap().deleted);
        assert!(
            file_repo::maybe_get_document(&db1, RepoSource::Local, file.id)
                .unwrap()
                .is_some()
        );
        assert!(file_repo::get_document(&db1, RepoSource::Local, file.id).is_ok());
    }

    #[test]
    // Test that documents are deleted after a sync
    fn delete_document_test_after_sync() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, &format!("{}/file.md", account.username)).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);
        sync!(&db2);

        assert!(metadata_repo::maybe_get(&db1, RepoSource::Local, file.id)
            .unwrap()
            .is_none());
        assert!(metadata_repo::maybe_get(&db2, RepoSource::Local, file.id)
            .unwrap()
            .is_none());

        assert!(
            file_repo::maybe_get_document(&db1, RepoSource::Local, file.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_document(&db2, RepoSource::Local, file.id)
                .unwrap()
                .is_none()
        );

        assert_eq!(file_repo::get_all_metadata_changes(&db1).unwrap().len(), 0);
        assert_eq!(file_repo::get_all_metadata_changes(&db2).unwrap().len(), 0);
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

        let file1_delete = path_service::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete = path_service::create_at_path(&db1, &path("delete/file2.md")).unwrap();
        let file3_delete = path_service::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = path_service::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = path_service::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = path_service::create_at_path(&db1, &path("stay/file3.md")).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);
        let to_delete = path_service::get_by_path(&db2, &path("delete")).unwrap();
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_dirty_ids!(db2, 1);
        sync!(&db2);

        // deleted files and their descendents are purged after sync
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );

        assert_dirty_ids!(db1, 1);
        sync!(&db1);

        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file3_stay.id)
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

        let file1_delete = path_service::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete = path_service::create_at_path(&db1, &path("delete/file2A.md")).unwrap();
        let file3_delete = path_service::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = path_service::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = path_service::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = path_service::create_at_path(&db1, &path("stay/file3.md")).unwrap();
        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file2_delete.id,
                file1_stay.parent,
            )
            .unwrap(),
        )
        .unwrap();
        let to_delete = path_service::get_by_path(&db2, &path("delete")).unwrap();
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder + moved document should show up as the sync instructions
        assert_dirty_ids!(db2, 2);
        sync!(&db2);

        // deleted files and their ancestors purged after sync
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_repo::maybe_get_metadata(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_repo::maybe_get_metadata(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );

        assert_dirty_ids!(db1, 2);
        sync!(&db1);

        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            file_repo::maybe_get_metadata(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !file_repo::maybe_get_metadata(&db1, RepoSource::Local, file3_stay.id)
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

        let file1_delete = path_service::create_at_path(&db1, &path("old/file1.md")).unwrap();
        let file2_delete = path_service::create_at_path(&db1, &path("old/file2.md")).unwrap();
        let file3_delete = path_service::create_at_path(&db1, &path("old/file3.md")).unwrap();
        let file4_delete = path_service::create_at_path(&db1, &path("old/file4.md")).unwrap();

        sync!(&db1);

        let new_folder = path_service::create_at_path(&db1, &path("new/")).unwrap();
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file2_delete.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_move(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file4_delete.id,
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                new_folder.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            !metadata_repo::maybe_get(&db1, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            metadata_repo::maybe_get(&db1, RepoSource::Local, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !metadata_repo::maybe_get(&db1, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            metadata_repo::maybe_get(&db1, RepoSource::Local, file4_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            metadata_repo::maybe_get(&db1, RepoSource::Local, new_folder.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        sync!(&db1);

        make_and_sync_new_client!(db2, db1);

        assert!(
            !metadata_repo::maybe_get(&db2, RepoSource::Local, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            metadata_repo::maybe_get(&db2, RepoSource::Local, file2_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !metadata_repo::maybe_get(&db2, RepoSource::Local, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            metadata_repo::maybe_get(&db2, RepoSource::Local, file4_delete.id)
                .unwrap()
                .is_none()
        );
        assert!(
            metadata_repo::maybe_get(&db2, RepoSource::Local, new_folder.id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn create_document_sync_delete_document_sync() {
        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = path_service::create_at_path(&db1, &path("file1.md")).unwrap();

        sync!(&db1);
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file1.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);
        assert_dirty_ids!(db1, 0);
    }

    #[test]
    fn deleted_path_is_released() {
        let db1 = test_config();
        let account = make_account!(db1);
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = path_service::create_at_path(&db1, &path("file1.md")).unwrap();
        sync!(&db1);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file1.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        path_service::create_at_path(&db1, &path("file1.md")).unwrap();
        sync!(&db1);
    }

    #[test]
    fn folder_delete_bug() {
        let db1 = test_config();
        let account = make_account!(db1);

        path_service::create_at_path(&db1, path!(account, "test/folder/document.md")).unwrap();
        sync!(&db1);
        make_and_sync_new_client!(db2, db1);

        let folder_to_delete = path_service::get_by_path(&db1, path!(account, "test")).unwrap();
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                folder_to_delete.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        sync!(&db2); // There was an error here
    }

    #[test]
    fn ensure_that_deleting_a_file_doesnt_make_it_show_up_in_work_calculated() {
        let db1 = test_config();
        let account = make_account!(db1);

        let file =
            path_service::create_at_path(&db1, path!(account, "test/folder/document.md")).unwrap();
        sync!(&db1);

        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file.id,
            )
            .unwrap(),
        )
        .unwrap();

        assert_dirty_ids!(&db1, 1);

        sync!(&db1);

        assert_dirty_ids!(&db1, 0);
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

        for i in 1..10 {
            path_service::create_at_path(&db1, &format!("{}/tmp/{}/", account.username, i))
                .unwrap();
        }

        sync!(&db1);
        sync!(&db2);

        let file_to_break = path_service::get_by_path(&db1, path!(account, "tmp")).unwrap();

        // 1 Client renames and syncs
        file_repo::insert_metadatum(
            &db1,
            RepoSource::Local,
            &file_service::apply_rename(
                &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
                file_to_break.id,
                "tmp2",
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db1);

        // Other deletes and syncs
        file_repo::insert_metadatum(
            &db2,
            RepoSource::Local,
            &file_service::apply_delete(
                &file_repo::get_all_metadata(&db2, RepoSource::Local).unwrap(),
                file_to_break.id,
            )
            .unwrap(),
        )
        .unwrap();
        sync!(&db2);
    }

    // #[test]
    // fn recreate_smail_bug_attempt_3() {
    //     let db1 = test_config();
    //     let account = make_account!(db1);

    //     let parent = path_service::create_at_path(&db1, path!(account, "tmp/")).unwrap();
    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &file_service::create(Folder, parent.id, "child", &account.username),
    //     )
    //     .unwrap();

    //     sync!(&db1);

    //     make_and_sync_new_client!(db2, db1);

    //     file_repo::insert_metadata(
    //         &db2,
    //         RepoSource::Local,
    //         &file_service::create(Folder, parent.id, "child2", &account.username),
    //     )
    //     .unwrap();
    //     let work = sync_service::calculate_work(&db2).unwrap().work_units; // 1 piece of work, the new child
    //     assert_n_work_units!(db2, 1);

    //     file_repo::insert_metadata(
    //         &db1,
    //         RepoSource::Local,
    //         &file_service::apply_delete(
    //             &file_repo::get_all_metadata(&db1, RepoSource::Local).unwrap(),
    //             parent.id,
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    //     sync!(&db1);

    //     for wu in work {
    //         sync_service::execute_work(&db2, &account, wu).unwrap_err();
    //     }

    //     // Uninstall and fresh sync
    //     let db3 = test_config();
    //     account_service::import_account(&db3, &account_service::export_account(&db1).unwrap())
    //         .unwrap();

    //     sync_service::sync(&db3, None).unwrap();
    //     assert_no_metadata_problems!(&db3);
    // }

    #[test]
    fn issue_734_bug() {
        let db1 = test_config();
        let account = make_account!(db1);

        path_service::create_at_path(&db1, &format!("{}/{}/", account.username, account.username))
            .unwrap();

        sync!(&db1);
    }
}
