#[cfg(test)]
mod sync_tests {
    use crate::{random_username, test_db};
    use lockbook_core::model::work_unit::WorkUnit;
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::service::file_service::FileService;
    use lockbook_core::service::sync_service::SyncService;
    use lockbook_core::{DefaultAccountService, DefaultFileService, DefaultSyncService};

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

        assert!(work_units
            .work_units
            .clone()
            .into_iter()
            .any(|wu| match wu {
                WorkUnit::PushNewDocument(doc) => doc == file,
                _ => false,
            }));

        assert!(work_units
            .work_units
            .clone()
            .into_iter()
            .any(|wu| match wu {
                WorkUnit::PushNewFolder(folder) => folder.name == "a",
                _ => false,
            }));

        assert!(work_units
            .work_units
            .clone()
            .into_iter()
            .any(|wu| match wu {
                WorkUnit::PushNewFolder(folder) => folder.name == "b",
                _ => false,
            }));

        assert!(work_units
            .work_units
            .clone()
            .into_iter()
            .any(|wu| match wu {
                WorkUnit::PushNewFolder(folder) => folder.name == "c",
                _ => false,
            }));
    }
}
