mod integration_test;

#[cfg(test)]
mod sync_tests {
    use crate::integration_test::{assert_dbs_eq, generate_account, test_db};
    use lockbook_core::model::work_unit::WorkUnit;
    use lockbook_core::repo::document_repo::DocumentRepo;
    use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
    use lockbook_core::repo::local_changes_repo::LocalChangesRepo;
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::service::file_service::FileService;
    use lockbook_core::service::sync_service::SyncService;
    use lockbook_core::{
        DefaultAccountService, DefaultDocumentRepo, DefaultFileMetadataRepo, DefaultFileService,
        DefaultLocalChangesRepo, DefaultSyncService,
    };

    #[test]
    fn test_create_files_and_folders_sync() {
        let generated_account = generate_account();
        let db = test_db();
        let account = DefaultAccountService::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            0
        );

        DefaultFileService::create_at_path(
            &db,
            format!("{}/a/b/c/test", account.username).as_str(),
        )
        .unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            4
        );

        assert!(DefaultSyncService::sync(&db).is_ok());

        let db2 = test_db();
        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db).unwrap(),
        )
        .unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            5
        );

        DefaultSyncService::sync(&db2).unwrap();
        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );

        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            0
        );
    }

    #[test]
    fn test_edit_document_sync() {
        let generated_account = generate_account();
        let db = test_db();
        let account = DefaultAccountService::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            0
        );
        println!("1st calculate work");

        let file = DefaultFileService::create_at_path(
            &db,
            format!("{}/a/b/c/test", account.username).as_str(),
        )
        .unwrap();

        assert!(DefaultSyncService::sync(&db).is_ok());
        println!("1st sync done");

        let db2 = test_db();
        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        println!("2nd sync done, db2");

        DefaultFileService::write_document(&db, file.id, "meaningful messages".as_bytes()).unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            1
        );
        println!("2nd calculate work, db1, 1 dirty file");

        match DefaultSyncService::calculate_work(&db)
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

        DefaultSyncService::sync(&db).unwrap();
        println!("3rd sync done, db1, dirty file pushed");

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            0
        );
        println!("4th calculate work, db1, dirty file pushed");

        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            1
        );
        println!("5th calculate work, db2, dirty file needs to be pulled");

        let edited_file = DefaultFileMetadataRepo::get(&db, file.id).unwrap();

        match DefaultSyncService::calculate_work(&db2)
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

        DefaultSyncService::sync(&db2).unwrap();
        println!("4th sync done, db2, dirty file pulled");
        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            0
        );
        println!("7th calculate work ");

        assert_eq!(
            DefaultFileService::read_document(&db2, edited_file.id).unwrap(),
            "meaningful messages".as_bytes()
        );
        assert_dbs_eq(&db, &db2);
    }

    #[test]
    fn test_move_document_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "nice document".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();

        assert_dbs_eq(&db1, &db2);

        let new_folder =
            DefaultFileService::create_at_path(&db1, &format!("{}/folder2/", account.username))
                .unwrap();

        DefaultFileService::move_file(&db1, file.id, new_folder.id).unwrap();
        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            2
        );

        DefaultSyncService::sync(&db1).unwrap();
        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            0
        );

        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            2
        );
        DefaultSyncService::sync(&db2).unwrap();
        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            0
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );

        assert_eq!(
            DefaultFileService::read_document(&db2, file.id).unwrap(),
            "nice document".as_bytes()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_move_reject() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "Wow, what a doc".as_bytes()).unwrap();

        let new_folder1 =
            DefaultFileService::create_at_path(&db1, &format!("{}/folder2/", account.username))
                .unwrap();

        let new_folder2 =
            DefaultFileService::create_at_path(&db1, &format!("{}/folder3/", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::move_file(&db2, file.id, new_folder1.id).unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::move_file(&db1, file.id, new_folder2.id).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert_dbs_eq(&db1, &db2);

        assert_eq!(
            DefaultFileMetadataRepo::get(&db1, file.id).unwrap().parent,
            new_folder1.id
        );
        assert_eq!(
            DefaultFileService::read_document(&db2, file.id).unwrap(),
            "Wow, what a doc".as_bytes()
        );
    }

    #[test]
    fn test_rename_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();

        DefaultFileService::rename_file(&db1, file.parent, "folder1-new").unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db2,
                &format!("{}/folder1-new", account.username)
            )
            .unwrap()
            .unwrap()
            .name,
            "folder1-new"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db2,
                &format!("{}/folder1-new/", account.username)
            )
            .unwrap()
            .unwrap()
            .name,
            "folder1-new"
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_rename_reject_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::rename_file(&db1, file.parent, "folder1-new").unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();
        DefaultFileService::rename_file(&db2, file.parent, "folder2-new").unwrap();
        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db2,
                &format!("{}/folder2-new", account.username)
            )
            .unwrap()
            .unwrap()
            .name,
            "folder2-new"
        );
        assert_eq!(
            DefaultFileMetadataRepo::get_by_path(
                &db2,
                &format!("{}/folder2-new/", account.username)
            )
            .unwrap()
            .unwrap()
            .name,
            "folder2-new"
        );
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn move_then_edit() {
        let db1 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/test.txt", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::rename_file(&db1, file.id, "new_name.txt").unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::write_document(&db1, file.id, "noice".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();
    }

    #[test]
    fn sync_fs_invalid_state_via_rename() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file1 =
            DefaultFileService::create_at_path(&db1, &format!("{}/test.txt", account.username))
                .unwrap();
        let file2 =
            DefaultFileService::create_at_path(&db1, &format!("{}/test2.txt", account.username))
                .unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::rename_file(&db2, file1.id, "test3.txt").unwrap();

        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::rename_file(&db1, file2.id, "test3.txt").unwrap();
        // Just operate on the server work
        DefaultSyncService::calculate_work(&db1)
            .unwrap()
            .work_units
            .into_iter()
            .filter(|work| match work {
                WorkUnit::LocalChange { .. } => false,
                WorkUnit::ServerChange { .. } => true,
            })
            .for_each(|work| DefaultSyncService::execute_work(&db1, &account, work).unwrap());

        println!(
            "{:#?}",
            DefaultFileMetadataRepo::test_repo_integrity(&db1).unwrap()
        );

        assert!(DefaultFileMetadataRepo::test_repo_integrity(&db1)
            .unwrap()
            .is_empty());

        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            1
        );

        DefaultSyncService::sync(&db1).unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn sync_fs_invalid_state_via_move() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file1 =
            DefaultFileService::create_at_path(&db1, &format!("{}/a/test.txt", account.username))
                .unwrap();
        let file2 =
            DefaultFileService::create_at_path(&db1, &format!("{}/b/test.txt", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();
        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::move_file(
            &db1,
            file1.id,
            DefaultFileMetadataRepo::get_root(&db1).unwrap().unwrap().id,
        )
        .unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::move_file(
            &db2,
            file2.id,
            DefaultFileMetadataRepo::get_root(&db2).unwrap().unwrap().id,
        )
        .unwrap();

        println!("{:#?}", DefaultFileMetadataRepo::get_all(&db2).unwrap());

        DefaultSyncService::calculate_work(&db2)
            .unwrap()
            .work_units
            .into_iter()
            .filter(|work| match work {
                WorkUnit::LocalChange { .. } => false,
                WorkUnit::ServerChange { .. } => true,
            })
            .for_each(|work| DefaultSyncService::execute_work(&db2, &account, work).unwrap());

        println!("{:#?}", DefaultFileMetadataRepo::get_all(&db2).unwrap());

        assert!(DefaultFileMetadataRepo::test_repo_integrity(&db2)
            .unwrap()
            .is_empty());

        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            0
        );

        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            1
        );

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_unmergable() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/test.bin", account.username))
                .unwrap();

        DefaultFileService::write_document(&db1, file.id, "some good content".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::write_document(&db1, file.id, "some new content".as_bytes()).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::write_document(&db2, file.id, "some offline content".as_bytes())
            .unwrap();
        let works = DefaultSyncService::calculate_work(&db2).unwrap();

        assert_eq!(works.work_units.len(), 2);

        for work in works.clone().work_units {
            DefaultSyncService::execute_work(&db2, &account, work).unwrap();
        }

        let works = DefaultSyncService::calculate_work(&db2).unwrap();
        assert_eq!(works.work_units.len(), 1);

        match works.work_units.get(0).unwrap() {
            WorkUnit::LocalChange { metadata } => {
                assert!(metadata.name.contains("CONTENT-CONFLICT"))
            }
            WorkUnit::ServerChange { .. } => panic!("This should not be the work type"),
        }

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_mergable() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/mergable_file.md", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        DefaultFileService::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes())
            .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_move_before_mergable() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/mergable_file.md", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        let folder =
            DefaultFileService::create_at_path(&db2, &format!("{}/folder1/", account.username))
                .unwrap();
        DefaultFileService::move_file(&db2, file.id, folder.id).unwrap();
        DefaultFileService::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes())
            .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_local_after_before_mergable() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/mergable_file.md", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        let folder =
            DefaultFileService::create_at_path(&db2, &format!("{}/folder1/", account.username))
                .unwrap();
        DefaultFileService::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes())
            .unwrap();
        DefaultFileService::move_file(&db2, file.id, folder.id).unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_content_conflict_server_after_before_mergable() {
        let db1 = test_db();
        let db2 = test_db();

        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/mergable_file.md", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\n".as_bytes()).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::write_document(&db1, file.id, "Line 1\nLine 2\n".as_bytes()).unwrap();
        let folder =
            DefaultFileService::create_at_path(&db1, &format!("{}/folder1/", account.username))
                .unwrap();
        DefaultFileService::move_file(&db1, file.id, folder.id).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        DefaultFileService::write_document(&db2, file.id, "Line 1\nOffline Line\n".as_bytes())
            .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 1"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Line 2"));
        assert!(String::from_utf8_lossy(
            &DefaultFileService::read_document(&db1, file.id).unwrap()
        )
        .contains("Offline Line"));
        assert_dbs_eq(&db1, &db2);
    }

    #[test]
    fn test_not_really_editing_should_not_cause_work() {
        let db = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultFileService::write_document(&db, file.id, "original".as_bytes()).unwrap();

        DefaultSyncService::sync(&db).unwrap();

        assert!(DefaultSyncService::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());

        DefaultFileService::write_document(&db, file.id, "original".as_bytes()).unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            0
        );
    }

    #[test]
    fn test_not_really_renaming_should_not_cause_work() {
        let db = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultSyncService::sync(&db).unwrap();

        assert!(DefaultSyncService::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());

        assert!(DefaultFileService::rename_file(&db, file.id, "file.md").is_err());

        assert!(DefaultSyncService::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());
    }

    #[test]
    fn test_not_really_moving_should_not_cause_work() {
        let db = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultSyncService::sync(&db).unwrap();

        assert!(DefaultSyncService::calculate_work(&db)
            .unwrap()
            .work_units
            .is_empty());

        assert!(DefaultFileService::move_file(&db, file.id, file.parent).is_err());
    }

    #[test]
    // Test that documents are deleted when a fresh sync happens
    fn delete_document_test_sync() {
        let db1 = test_db();
        let db2 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();
        DefaultFileService::delete_document(&db1, file.id).unwrap();
        assert!(DefaultFileMetadataRepo::get(&db1, file.id).unwrap().deleted);
        DefaultSyncService::sync(&db1).unwrap();
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());
        DefaultSyncService::sync(&db2).unwrap();
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());

        assert!(DefaultFileService::read_document(&db2, file.id).is_err());
    }

    #[test]
    fn delete_new_document_never_synced() {
        let db1 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultFileService::delete_document(&db1, file.id).unwrap();
        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            0
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileService::read_document(&db1, file.id).is_err());
    }

    #[test]
    // Test that documents are deleted after a sync
    fn delete_document_test_after_sync() {
        let db1 = test_db();
        let db2 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/file.md", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        DefaultFileService::delete_document(&db1, file.id).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        DefaultSyncService::sync(&db2).unwrap();

        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());

        assert!(DefaultDocumentRepo::maybe_get(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(DefaultDocumentRepo::maybe_get(&db2, file.id)
            .unwrap()
            .is_none());

        assert!(DefaultLocalChangesRepo::get_local_changes(&db1, file.id)
            .unwrap()
            .is_none());
        assert!(DefaultLocalChangesRepo::get_local_changes(&db2, file.id)
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

        let db1 = test_db();
        let db2 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file2.md")).unwrap();
        let file3_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = DefaultFileService::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = DefaultFileService::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = DefaultFileService::create_at_path(&db1, &path("stay/file3.md")).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultFileService::delete_folder(
            &db2,
            DefaultFileMetadataRepo::get_by_path(&db2, &path("delete"))
                .unwrap()
                .unwrap()
                .id,
        )
        .unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            1
        );
        DefaultSyncService::sync(&db2).unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .is_none()
        );

        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            4
        );
        DefaultSyncService::sync(&db1).unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db1, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file3_stay.id)
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

        let db1 = test_db();
        let db2 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file1.md")).unwrap();
        let file2_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file2A.md")).unwrap();
        let file3_delete =
            DefaultFileService::create_at_path(&db1, &path("delete/file3.md")).unwrap();

        let file1_stay = DefaultFileService::create_at_path(&db1, &path("stay/file1.md")).unwrap();
        let file2_stay = DefaultFileService::create_at_path(&db1, &path("stay/file2.md")).unwrap();
        let file3_stay = DefaultFileService::create_at_path(&db1, &path("stay/file3.md")).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();
        DefaultFileService::move_file(&db2, file2_delete.id, file1_stay.parent).unwrap();
        DefaultFileService::delete_folder(
            &db2,
            DefaultFileMetadataRepo::get_by_path(&db2, &path("delete"))
                .unwrap()
                .unwrap()
                .id,
        )
        .unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        // Only the folder should show up as the sync instruction
        assert_eq!(
            DefaultSyncService::calculate_work(&db2)
                .unwrap()
                .work_units
                .len(),
            2
        );
        DefaultSyncService::sync(&db2).unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.parent)
                .unwrap()
                .is_none()
        );

        assert_eq!(
            DefaultSyncService::calculate_work(&db1)
                .unwrap()
                .work_units
                .len(),
            4
        );
        DefaultSyncService::sync(&db1).unwrap();

        assert!(
            DefaultFileMetadataRepo::maybe_get(&db1, file1_delete.parent)
                .unwrap()
                .is_none()
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file1_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db1, file3_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file1_stay.parent)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file1_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file2_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file3_stay.id)
                .unwrap()
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn create_new_folder_and_move_old_files_into_it_then_delete_that_folder() {
        let db1 = test_db();
        let db2 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1_delete = DefaultFileService::create_at_path(&db1, &path("old/file1.md")).unwrap();
        let file2_delete = DefaultFileService::create_at_path(&db1, &path("old/file2.md")).unwrap();
        let file3_delete = DefaultFileService::create_at_path(&db1, &path("old/file3.md")).unwrap();
        let file4_delete = DefaultFileService::create_at_path(&db1, &path("old/file4.md")).unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        let new_folder = DefaultFileService::create_at_path(&db1, &path("new/")).unwrap();
        DefaultFileService::move_file(&db1, file2_delete.id, new_folder.id).unwrap();
        DefaultFileService::move_file(&db1, file4_delete.id, new_folder.id).unwrap();
        DefaultFileService::delete_folder(&db1, new_folder.id).unwrap();

        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            DefaultFileMetadataRepo::maybe_get(&db1, file2_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db1, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            DefaultFileMetadataRepo::maybe_get(&db1, file4_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(
            DefaultFileMetadataRepo::maybe_get(&db1, new_folder.id)
                .unwrap()
                .unwrap()
                .deleted
        );

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();

        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file1_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file2_delete.id)
            .unwrap()
            .is_none());
        assert!(
            !DefaultFileMetadataRepo::maybe_get(&db2, file3_delete.id)
                .unwrap()
                .unwrap()
                .deleted
        );
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, file4_delete.id)
            .unwrap()
            .is_none());
        assert!(DefaultFileMetadataRepo::maybe_get(&db2, new_folder.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn create_document_sync_delete_document_sync() {
        let db1 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = DefaultFileService::create_at_path(&db1, &path("file1.md")).unwrap();

        DefaultSyncService::sync(&db1).unwrap();
        DefaultFileService::delete_document(&db1, file1.id).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
        assert!(DefaultSyncService::calculate_work(&db1)
            .unwrap()
            .work_units
            .is_empty());
    }

    #[test]
    fn deleted_path_is_released() {
        let db1 = test_db();
        let generated_account = generate_account();
        let account = DefaultAccountService::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let path = |path: &str| -> String { format!("{}/{}", &account.username, path) };

        let file1 = DefaultFileService::create_at_path(&db1, &path("file1.md")).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::delete_document(&db1, file1.id).unwrap();
        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::create_at_path(&db1, &path("file1.md")).unwrap();
        DefaultSyncService::sync(&db1).unwrap();
    }
}
