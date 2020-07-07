#[cfg(test)]
mod sync_tests {
    use crate::{random_username, test_db};
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
    }
}
