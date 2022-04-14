#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod account_tests {
    use crate::test_utils::{random_name, test_core, url};
    use lockbook_core::model::errors::{CreateAccountError, ImportError};
    use lockbook_core::repo::schema::OneKey;
    use lockbook_core::Error;
    use lockbook_crypto::pubkey;
    use lockbook_models::account::Account;

    #[test]
    fn create_account_success() {
        let core = test_core();
        core.create_account(&random_name(), &url()).unwrap();
    }

    #[test]
    fn create_account_username_taken() {
        let core1 = test_core();
        let core2 = test_core();
        let name = random_name();

        core1.create_account(&name, &url()).unwrap();

        let err = core2.create_account(&name, &url()).unwrap_err();

        assert!(
            matches!(err, Error::UiError(CreateAccountError::UsernameTaken)),
            "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
            &name,
            err
        )
    }

    #[test]
    fn create_account_invalid_username() {
        let core = test_core();

        let invalid_unames = ["", "i/o", "@me", "###", "+1", "💩"];

        for &uname in &invalid_unames {
            let err = core.create_account(uname, &url()).unwrap_err();

            assert!(
                matches!(err, Error::UiError(CreateAccountError::InvalidUsername)),
                "Username \"{}\" should have been InvalidUsername but instead was {:?}",
                uname,
                err
            )
        }
    }

    #[test]
    fn create_account_account_exists() {
        let core = &test_core();

        core.create_account(&random_name(), &url()).unwrap();

        assert!(
            matches!(
                core.create_account(&random_name(), &url()),
                Err(Error::UiError(CreateAccountError::AccountExistsAlready))
            ),
            "This action should have failed with AccountAlreadyExists!",
        );
    }

    #[test]
    fn create_account_account_exists_case() {
        let core = test_core();
        let name = random_name();

        core.create_account(&name, &url()).unwrap();

        let core = test_core();
        assert!(matches!(
            core.create_account(&(name.to_uppercase()), &url()),
            Err(Error::UiError(CreateAccountError::UsernameTaken))
        ));
    }

    #[test]
    fn import_account_account_exists() {
        let core = test_core();

        core.create_account(&random_name(), &url()).unwrap();
        let account_string = core.export_account().unwrap();

        assert!(matches!(
            core.import_account(&account_string),
            Err(Error::UiError(ImportError::AccountExistsAlready))
        ));
    }

    #[test]
    fn import_account_corrupted() {
        let core = test_core();

        assert!(matches!(
            core.import_account("clearly a bad account string"),
            Err(Error::UiError(ImportError::AccountStringCorrupted))
        ));
    }

    #[test]
    fn import_account_nonexistent() {
        let core1 = test_core();

        core1.create_account(&random_name(), &url()).unwrap();

        let core2 = test_core();
        let account = Account {
            api_url: url(),
            username: random_name(),
            private_key: pubkey::generate_key(),
        };
        core2.db.account.insert(OneKey {}, account).unwrap();
        let account_string = core2.export_account().unwrap();

        let core3 = test_core();
        assert!(matches!(
            core3.import_account(&account_string),
            Err(Error::UiError(ImportError::AccountDoesNotExist))
        ));
    }

    #[test]
    fn import_account_public_key_mismatch() {
        let bad_account_string = {
            let core1 = test_core();
            let core2 = test_core();
            let account1 = core1.create_account(&random_name(), &url()).unwrap();
            let mut account2 = core2.create_account(&random_name(), &url()).unwrap();
            account2.username = account1.username;
            core2.db.account.insert(OneKey {}, account2).unwrap();
            core2.export_account().unwrap()
        };

        let core3 = test_core();

        assert!(matches!(
            core3.import_account(&bad_account_string),
            Err(Error::UiError(ImportError::UsernamePKMismatch))
        ));
    }
}
