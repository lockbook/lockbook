#[cfg(test)]
mod sync_tests {
    use crate::{random_username, test_db};
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::service::file_service::FileService;
    use lockbook_core::{DefaultAccountService, DefaultFileService, DefaultSyncService};
    use lockbook_core::service::sync_service::SyncService;

    #[test]
    fn new_file_sync() {
        let db = test_db();
        let account = DefaultAccountService::create_account(&db, &random_username()).unwrap();
        let file = DefaultFileService::create_at_path(
            &db,
            format!("{}/a/b/c/test", account.username).as_str(),
        )
        .unwrap();

        let work_units = DefaultSyncService::calculate_work(&db).unwrap();

        assert_eq!(work_units.most_recent_update_from_server, 0);
    }
}
