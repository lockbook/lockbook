#[cfg(test)]
mod sync_tests {
    use crate::{random_username, test_db};
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::service::file_service::FileService;
    use lockbook_core::service::sync_service::SyncService;
    use lockbook_core::{
        init_logger_safely, DefaultAccountService, DefaultFileService, DefaultSyncService,
    };

    #[test]
    fn test_new_account_sync() {
        init_logger_safely();
        let db = test_db();
        let account = DefaultAccountService::create_account(&db, &random_username()).unwrap();

        assert_eq!(
            DefaultSyncService::calculate_work(&db)
                .unwrap()
                .work_units
                .len(),
            0
        );

        let file = DefaultFileService::create_at_path(
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
    }
}
