mod integration_test;

#[cfg(test)]
mod account_tests {
    use lockbook_core::repo::account_repo;
    use lockbook_core::service::account_service;
    use lockbook_core::service::test_utils::{generate_account, random_username, test_config};
    use lockbook_core::{
        create_account, export_account, import_account, CoreError, Error, ImportError,
    };
    use lockbook_models::account::Account;

    #[test]
    fn create_account_success() {
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
    fn create_account_username_taken() {
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
    fn create_account_invalid_username() {
        let db = test_config();

        let invalid_unames = ["", "i/o", "@me", "###", "+1", "💩"];

        for &uname in &invalid_unames {
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
    fn create_account_account_exists() {
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
    fn import_account_account_exists() {
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
    fn import_account_corrupted() {
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
    fn import_account_nonexistent() {
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
            account_repo::insert(&cfg2, &account).unwrap();
        } // release lock on db

        let account_string = export_account(&cfg2).unwrap();

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
    fn import_account_public_key_mismatch() {
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
            account_repo::insert(&db2, &account2).unwrap();
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
