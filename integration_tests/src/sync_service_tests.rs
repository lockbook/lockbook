#[cfg(test)]
mod sync_tests {
    use crate::{random_username, test_db};
    use lockbook_core::model::crypto::DecryptedValue;
    use lockbook_core::model::work_unit::WorkUnit;
    use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::service::file_service::FileService;
    use lockbook_core::service::sync_service::SyncService;
    use lockbook_core::{
        DefaultAccountService, DefaultFileMetadataRepo, DefaultFileService, DefaultSyncService,
    };

    #[test]
    fn test_create_files_and_folders_sync() {
        let db = test_db();
        let account = DefaultAccountService::create_account(&db, &random_username()).unwrap();

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
        let db = test_db();
        let account = DefaultAccountService::create_account(&db, &random_username()).unwrap();

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

        DefaultFileService::write_document(
            &db,
            file.id,
            &DecryptedValue::from("meaningful messages"),
        )
        .unwrap();

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
            DefaultFileService::read_document(&db2, edited_file.id)
                .unwrap()
                .secret,
            "meaningful messages".to_string()
        );
        assert_eq!(&db.checksum().unwrap(), &db2.checksum().unwrap());
    }

    #[test]
    fn test_move_document_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, &DecryptedValue::from("nice document"))
            .unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultAccountService::import_account(
            &db2,
            &DefaultAccountService::export_account(&db1).unwrap(),
        )
        .unwrap();

        DefaultSyncService::sync(&db2).unwrap();

        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );
        assert_eq!(&db1.checksum().unwrap(), &db2.checksum().unwrap());

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
        assert_ne!(&db1.checksum().unwrap(), &db2.checksum().unwrap());

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
            DefaultFileService::read_document(&db2, file.id)
                .unwrap()
                .secret,
            "nice document"
        );

        assert_eq!(&db1.checksum().unwrap(), &db2.checksum().unwrap());
    }

    #[test]
    fn test_move_reject() {
        let db1 = test_db();
        let db2 = test_db();

        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

        let file = DefaultFileService::create_at_path(
            &db1,
            &format!("{}/folder1/test.txt", account.username),
        )
        .unwrap();

        DefaultFileService::write_document(&db1, file.id, &DecryptedValue::from("Wow, what a doc"))
            .unwrap();

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

        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );

        assert_eq!(&db1.checksum().unwrap(), &db2.checksum().unwrap());

        assert_eq!(
            DefaultFileMetadataRepo::get(&db1, file.id).unwrap().parent,
            new_folder1.id
        );
        assert_eq!(
            DefaultFileService::read_document(&db2, file.id)
                .unwrap()
                .secret,
            "Wow, what a doc"
        );
    }

    #[test]
    fn test_rename_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

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
        assert_eq!(&db1.checksum().unwrap(), &db2.checksum().unwrap());
    }

    #[test]
    fn test_rename_reject_sync() {
        let db1 = test_db();
        let db2 = test_db();

        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

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
        assert_eq!(&db1.checksum().unwrap(), &db2.checksum().unwrap());
    }

    #[test]
    fn move_then_edit() {
        let db1 = test_db();

        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

        let file =
            DefaultFileService::create_at_path(&db1, &format!("{}/test.txt", account.username))
                .unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::rename_file(&db1, file.id, "new_name.txt").unwrap();

        DefaultSyncService::sync(&db1).unwrap();

        DefaultFileService::write_document(&db1, file.id, &DecryptedValue::from("noice")).unwrap();

        DefaultSyncService::sync(&db1).unwrap();
    }
}
