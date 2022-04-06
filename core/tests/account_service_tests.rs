#[cfg(test)]
mod account_tests {
    use lockbook_core::model::errors::{CreateAccountError, ImportError};

    use lockbook_core::repo::schema::OneKey;

    use lockbook_core::service::test_utils::{
        generate_account, random_username, test_config, test_core,
    };
    use lockbook_core::{Error, LbCore};
    use lockbook_models::account::Account;

    #[test]
    fn create_account_success() {
        let core = LbCore::init(&test_config()).unwrap();
        let generated_account = generate_account();
        core.create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();
    }

    #[test]
    fn create_account_username_taken() {
        let core1 = test_core();
        let core2 = test_core();
        let generated_account = generate_account();

        core1
            .create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();

        let err = core2
            .create_account(&generated_account.username, &generated_account.api_url)
            .unwrap_err();

        assert!(
            matches!(err, Error::UiError(CreateAccountError::UsernameTaken)),
            "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
            &generated_account.username,
            err
        )
    }

    #[test]
    fn create_account_invalid_username() {
        let core = test_core();

        let invalid_unames = ["", "i/o", "@me", "###", "+1", "💩"];

        for &uname in &invalid_unames {
            let err = core
                .create_account(uname, &generate_account().api_url)
                .unwrap_err();

            assert!(
                matches!(err, Error::UiError(CreateAccountError::UsernameTaken)),
                "Username \"{}\" should have been InvalidUsername but instead was {:?}",
                uname,
                err
            )
        }
    }

    #[test]
    fn create_account_account_exists() {
        let core = &test_core();
        let generated_account = generate_account();

        core.create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();

        assert!(
            matches!(
                core.create_account(&generated_account.username, &generated_account.api_url),
                Err(Error::UiError(CreateAccountError::AccountExistsAlready))
            ),
            "This action should have failed with AccountAlreadyExists!",
        );
    }

    #[test]
    fn create_account_account_exists_case() {
        let core = test_core();
        let generated_account = generate_account();

        core.create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();

        let core = test_core();
        assert!(
            matches!(
                core.create_account(
                    &(generated_account.username.to_uppercase()),
                    &generated_account.api_url
                ),
                Err(Error::UiError(CreateAccountError::UsernameTaken))
            ),
            "This action should have failed with AccountAlreadyExists!",
        );
        println!("{} {}", &generated_account.username, &(generated_account.username.to_uppercase()))
    }

    #[test]
    fn import_account_account_exists() {
        let core = test_core();
        let generated_account = generate_account();

        core.create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();
        let account_string = core.export_account().unwrap();

        match core.import_account(&account_string) {
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
        let core = test_core();

        match core.import_account("clearly a bad account string") {
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
        let core1 = test_core();
        let generated_account = generate_account();

        core1
            .create_account(&generated_account.username, &generated_account.api_url)
            .unwrap();

        let core2 = test_core();
        {
            let account = Account {
                api_url: generated_account.api_url,
                username: random_username(),
                private_key: generated_account.private_key,
            };
            core2.db.account.insert(OneKey {}, account).unwrap();
        } // release lock on db

        let account_string = core2.export_account().unwrap();

        let core3 = test_core();

        match core3.import_account(&account_string) {
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
            let core1 = test_core();
            let core2 = test_core();
            let generated_account1 = generate_account();
            let generated_account2 = generate_account();
            let account1 = core1
                .create_account(&generated_account1.username, &generated_account1.api_url)
                .unwrap();
            let mut account2 = core2
                .create_account(&generated_account2.username, &generated_account2.api_url)
                .unwrap();
            account2.username = account1.username;
            core2.db.account.insert(OneKey {}, account2).unwrap();
            core2.export_account().unwrap()
        };

        let core3 = test_core();

        match core3.import_account(&bad_account_string) {
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
