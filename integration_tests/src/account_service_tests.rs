#[cfg(test)]
mod account_tests {
    use lockbook_core::client::Error;
    use lockbook_core::model::api::NewAccountError::UsernameTaken;
    use lockbook_core::service::account_service::{AccountCreationError, AccountService};
    use lockbook_core::{
        DefaultAccountRepo, DefaultAccountService, DefaultFileMetadataRepo, DefaultSyncService,
    };

    use crate::{random_username, test_db};
    use lockbook_core::model::api::NewAccountError;
    use lockbook_core::repo::account_repo::AccountRepo;
    use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
    use lockbook_core::service::sync_service::SyncService;

    #[test]
    fn create_account_successfully() {
        let db = test_db();
        DefaultAccountService::create_account(&db, &random_username()).unwrap();
    }

    #[test]
    fn username_taken_test() {
        let db = test_db();
        let username = &random_username();
        DefaultAccountService::create_account(&db, username).unwrap();

        match DefaultAccountService::create_account(&db, username).unwrap_err() {
            AccountCreationError::ApiError(api_err) => match api_err {
                Error::Api(api_api_err) => {
                    match api_api_err {
                        UsernameTaken => {
                            return; // Test passed
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        };

        panic!("This username should have been taken.")
    }

    #[test]
    fn invalid_username_test() {
        let db = test_db();
        match DefaultAccountService::create_account(&db, "💩").unwrap_err() {
            AccountCreationError::ApiError(api_err) => match api_err {
                Error::Api(api_api_err) => {
                    match api_api_err {
                        NewAccountError::InvalidUsername => return, // Test passed
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        }

        panic!("This username should have been invalid.")
    }

    #[test]
    fn import_sync() {
        let db1 = test_db();
        let account = DefaultAccountService::create_account(&db1, &random_username()).unwrap();

        let account_string = DefaultAccountService::export_account(&db1).unwrap();
        let home_folders1 = DefaultFileMetadataRepo::get_root(&db1).unwrap().unwrap();

        let db2 = test_db();
        assert!(DefaultAccountService::export_account(&db2).is_err());
        assert!(DefaultAccountService::import_account(&db2, &account_string).is_ok());
        assert_eq!(DefaultAccountRepo::get_account(&db2).unwrap(), account);
        assert_eq!(DefaultFileMetadataRepo::get_last_updated(&db2).unwrap(), 0);

        let work = DefaultSyncService::calculate_work(&db2).unwrap();
        assert_ne!(work.most_recent_update_from_server, 0);
        assert_eq!(work.work_units.len(), 1);
        assert!(DefaultFileMetadataRepo::get_root(&db2).unwrap().is_none());
        DefaultSyncService::sync(&db2).unwrap();
        assert!(DefaultFileMetadataRepo::get_root(&db2).unwrap().is_some());
        let home_folders2 = DefaultFileMetadataRepo::get_root(&db2).unwrap().unwrap();
        assert_eq!(home_folders1, home_folders2);
        assert_eq!(
            DefaultFileMetadataRepo::get_all(&db1).unwrap(),
            DefaultFileMetadataRepo::get_all(&db2).unwrap()
        );
    }
}
