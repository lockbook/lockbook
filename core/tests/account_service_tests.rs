mod integration_test;

#[cfg(test)]
mod account_tests {
    use lockbook_core::repo::{account_repo, file_metadata_repo};
    use lockbook_core::service::test_utils::{generate_account, random_username, test_config};
    use lockbook_core::service::{account_service, sync_service};
    use lockbook_core::{
        create_account, export_account, import_account, CoreError, Error, ImportError,
    };
    use lockbook_models::account::Account;

    #[test]
    fn create_account_successfully() {
        let db = test_config();
        let generated_account = generate_account();
        account_service::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
    }

    #[test]
    fn username_taken_test() {
        let db1 = test_config();
        let db2 = test_config();
        let generated_account = generate_account();
        account_service::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let err = account_service::create_account(
            &db2,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap_err();

        assert!(
            matches!(err, CoreError::UsernameTaken),
            "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
            &generated_account.username,
            err
        )
    }

    #[test]
    fn invalid_username_test() {
        let db = test_config();

        let invalid_unames = ["", "i/o", "@me", "###", "+1", "💩"];

        for uname in &invalid_unames {
            let err = account_service::create_account(&db, uname, &generate_account().api_url)
                .unwrap_err();

            assert!(
                matches!(err, CoreError::UsernameInvalid),
                "Username \"{}\" should have been InvalidUsername but instead was {:?}",
                uname,
                err
            )
        }
    }

    #[test]
    fn import_sync() {
        let db1 = test_config();
        let generated_account = generate_account();
        let account = account_service::create_account(
            &db1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let account_string = account_service::export_account(&db1).unwrap();
        let home_folders1 = file_metadata_repo::get_root(&db1).unwrap().unwrap();

        let db2 = test_config();
        assert!(account_service::export_account(&db2).is_err());
        account_service::import_account(&db2, &account_string).unwrap();
        assert_eq!(account_repo::get_account(&db2).unwrap(), account);
        assert_eq!(file_metadata_repo::get_last_updated(&db2).unwrap(), 0);

        let work = sync_service::calculate_work(&db2).unwrap();
        assert_ne!(work.most_recent_update_from_server, 0);
        assert_eq!(work.work_units.len(), 1);
        assert!(file_metadata_repo::get_root(&db2).unwrap().is_none());
        sync_service::sync(&db2, None).unwrap();
        assert!(file_metadata_repo::get_root(&db2).unwrap().is_some());
        let home_folders2 = file_metadata_repo::get_root(&db2).unwrap().unwrap();
        assert_eq!(home_folders1, home_folders2);
        assert_eq!(
            file_metadata_repo::get_all(&db1).unwrap(),
            file_metadata_repo::get_all(&db2).unwrap()
        );
    }

    #[test]
    fn test_new_account_when_one_exists() {
        let db = test_config();
        let generated_account = generate_account();

        account_service::create_account(
            &db,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        assert!(
            matches!(
                account_service::create_account(
                    &db,
                    &generated_account.username,
                    &generated_account.api_url,
                ),
                Err(CoreError::AccountExists)
            ),
            "This action should have failed with AccountAlreadyExists!",
        );
    }

    #[test]
    fn test_import_account_when_one_exists() {
        let cfg1 = test_config();
        let generated_account = generate_account();

        create_account(
            &cfg1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let account_string = export_account(&cfg1).unwrap();

        match import_account(&cfg1, &account_string) {
            Ok(_) => panic!(
                "This should not have allowed this account to be imported as one exists already"
            ),
            Err(err) => match err {
                Error::UiError(ImportError::AccountExistsAlready) => {}
                Error::UiError(ImportError::AccountStringCorrupted)
                | Error::UiError(ImportError::AccountDoesNotExist)
                | Error::UiError(ImportError::UsernamePKMismatch)
                | Error::UiError(ImportError::ClientUpdateRequired)
                | Error::UiError(ImportError::CouldNotReachServer)
                | Error::Unexpected(_) => panic!("Wrong Error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_account_string_corrupted() {
        let cfg1 = test_config();

        match import_account(&cfg1, "clearly a bad account string") {
            Ok(_) => panic!("This should not be a valid account string"),
            Err(err) => match err {
                Error::UiError(ImportError::AccountStringCorrupted) => {}
                Error::UiError(ImportError::AccountExistsAlready)
                | Error::UiError(ImportError::AccountDoesNotExist)
                | Error::UiError(ImportError::UsernamePKMismatch)
                | Error::UiError(ImportError::ClientUpdateRequired)
                | Error::UiError(ImportError::CouldNotReachServer)
                | Error::Unexpected(_) => panic!("Wrong Error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_importing_nonexistent_account() {
        let cfg1 = test_config();
        let generated_account = generate_account();

        create_account(
            &cfg1,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();

        let cfg2 = test_config();
        {
            let account = Account {
                api_url: generated_account.api_url,
                username: random_username(),
                private_key: generated_account.private_key,
            };
            account_repo::insert_account(&cfg2, &account).unwrap();
        } // release lock on db

        let account_string = export_account(&cfg2).unwrap();

        println!("Your thing\n{}", &account_string);

        let cfg3 = test_config();

        match import_account(&cfg3, &account_string) {
            Ok(_) => panic!("Should not have passed"),
            Err(err) => match err {
                Error::UiError(ImportError::AccountDoesNotExist) => {}
                Error::UiError(ImportError::AccountStringCorrupted)
                | Error::UiError(ImportError::AccountExistsAlready)
                | Error::UiError(ImportError::ClientUpdateRequired)
                | Error::UiError(ImportError::UsernamePKMismatch)
                | Error::UiError(ImportError::CouldNotReachServer)
                | Error::Unexpected(_) => panic!("Wrong error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_account_public_key_mismatch_import() {
        let bad_account_string = {
            let db1 = test_config();
            let db2 = test_config();
            let generated_account1 = generate_account();
            let generated_account2 = generate_account();
            let account1 = account_service::create_account(
                &db1,
                &generated_account1.username,
                &generated_account1.api_url,
            )
            .unwrap();
            let mut account2 = account_service::create_account(
                &db2,
                &generated_account2.username,
                &generated_account2.api_url,
            )
            .unwrap();
            account2.username = account1.username;
            account_repo::insert_account(&db2, &account2).unwrap();
            account_service::export_account(&db2).unwrap()
        };

        match import_account(&test_config(), &bad_account_string) {
            Ok(_) => panic!("Should have failed"),
            Err(err) => match err {
                Error::UiError(ImportError::UsernamePKMismatch) => {}
                Error::UiError(ImportError::AccountStringCorrupted)
                | Error::UiError(ImportError::AccountExistsAlready)
                | Error::UiError(ImportError::ClientUpdateRequired)
                | Error::UiError(ImportError::AccountDoesNotExist)
                | Error::UiError(ImportError::CouldNotReachServer)
                | Error::Unexpected(_) => panic! {"Wrong error: {:#?}", err},
            },
        }
    }
}
