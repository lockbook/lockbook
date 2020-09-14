mod integration_test;

#[cfg(test)]
mod account_tests {
    use lockbook_core::client::Error;
    use lockbook_core::service::account_service::{
        AccountCreationError, AccountImportError, AccountService,
    };
    use lockbook_core::{
        create_account, export_account, import_account, DefaultAccountRepo, DefaultAccountService,
        DefaultDbProvider, DefaultFileMetadataRepo, DefaultSyncService, ImportError,
    };

    use crate::integration_test::{random_username, test_config, test_db};
    use lockbook_core::model::account::Account;
    use lockbook_core::model::api::NewAccountError;
    use lockbook_core::repo::account_repo::AccountRepo;
    use lockbook_core::repo::db_provider::DbProvider;
    use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
    use lockbook_core::service::sync_service::SyncService;
    use rsa::{BigUint, RSAPrivateKey};
    use std::mem::discriminant;

    #[test]
    fn create_account_successfully() {
        let db = test_db();
        DefaultAccountService::create_account(&db, &random_username()).unwrap();
    }

    #[test]
    fn username_taken_test() {
        let db1 = test_db();
        let db2 = test_db();
        let username = &random_username();
        DefaultAccountService::create_account(&db1, username).unwrap();

        let err = DefaultAccountService::create_account(&db2, username).unwrap_err();

        assert!(
            matches!(
                err,
                AccountCreationError::ApiError(Error::Api(NewAccountError::UsernameTaken))
            ),
            "Username \"{}\" should have caused a UsernameTaken error but instead was {:?}",
            username,
            err
        )
    }

    #[test]
    fn invalid_username_test() {
        let db = test_db();
        let invalid_unames = ["", "i/o", "@me", "###", "+1", "💩"];

        for uname in &invalid_unames {
            let err = DefaultAccountService::create_account(&db, uname).unwrap_err();

            assert!(
                matches!(
                    err,
                    AccountCreationError::ApiError(Error::Api(NewAccountError::InvalidUsername))
                ),
                "Username \"{}\" should have been InvalidUsername but instead was {:?}",
                uname,
                err
            )
        }
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

    #[test]
    fn test_new_account_when_one_exists() {
        let db = test_db();
        let username = &random_username();

        DefaultAccountService::create_account(&db, username).unwrap();
        match DefaultAccountService::create_account(&db, username) {
            Ok(_) => panic!("This action should have failed with AccountAlreadyExists!"),
            Err(err) => match err {
                AccountCreationError::KeyGenerationError(_)
                | AccountCreationError::AccountRepoError(_)
                | AccountCreationError::AccountRepoDbError(_)
                | AccountCreationError::FolderError(_)
                | AccountCreationError::MetadataRepoError(_)
                | AccountCreationError::ApiError(_)
                | AccountCreationError::KeySerializationError(_)
                | AccountCreationError::AuthGenFailure(_) => {
                    panic!("This action should have failed with AccountAlreadyExists!")
                }
                AccountCreationError::AccountExistsAlready => println!("Success."),
            },
        }
    }

    #[test]
    fn test_import_invalid_private_key() {
        let db1 = test_db();
        let db2 = test_db();

        let account = Account {
            username: "Smail".to_string(),
            keys: RSAPrivateKey::from_components(
                BigUint::from_bytes_be(b"Test"),
                BigUint::from_bytes_be(b"Test"),
                BigUint::from_bytes_be(b"Test"),
                vec![
                    BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
                    BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
                ],
            ),
        };

        DefaultAccountRepo::insert_account(&db1, &account).unwrap();

        let result = discriminant(
            &DefaultAccountService::import_account(
                &db2,
                &DefaultAccountService::export_account(&db1).unwrap(),
            )
            .unwrap_err(),
        );
        let err = discriminant(&AccountImportError::InvalidPrivateKey(
            rsa::errors::Error::InvalidModulus,
        ));

        assert_eq!(result, err)
    }

    #[test]
    fn test_import_account_when_one_exists() {
        let cfg1 = test_config();

        create_account(&cfg1, &random_username()).unwrap();
        let account_string = export_account(&cfg1).unwrap();

        match import_account(&cfg1, &account_string) {
            Ok(_) => panic!(
                "This should not have allowed this account to be imported as one exists already"
            ),
            Err(err) => match err {
                ImportError::AccountExistsAlready => println!("Test passed!"),
                ImportError::AccountStringCorrupted
                | ImportError::AccountDoesNotExist
                | ImportError::UsernamePKMismatch
                | ImportError::ClientUpdateRequired
                | ImportError::CouldNotReachServer
                | ImportError::UnexpectedError(_) => panic!("Wrong Error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_account_string_corrupted() {
        let cfg1 = test_config();

        match import_account(&cfg1, "clearly a bad account string") {
            Ok(_) => panic!("This should not be a valid account string"),
            Err(err) => match err {
                ImportError::AccountStringCorrupted => println!("Test passed!"),
                ImportError::AccountExistsAlready
                | ImportError::AccountDoesNotExist
                | ImportError::UsernamePKMismatch
                | ImportError::ClientUpdateRequired
                | ImportError::CouldNotReachServer
                | ImportError::UnexpectedError(_) => panic!("Wrong Error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_importing_nonexistent_account() {
        let cfg1 = test_config();

        create_account(&cfg1, &random_username()).unwrap();

        {
            let db = DefaultDbProvider::connect_to_db(&cfg1).unwrap();
            let mut account = DefaultAccountRepo::get_account(&db).unwrap();
            account.username = random_username();
            DefaultAccountRepo::insert_account(&db, &account).unwrap();
        } // release lock on db

        let account_string = export_account(&cfg1).unwrap();

        let cfg2 = test_config();

        match import_account(&cfg2, &account_string) {
            Ok(_) => panic!("Should not have passed"),
            Err(err) => match err {
                ImportError::AccountDoesNotExist => println!("Test passed!"),
                ImportError::AccountStringCorrupted
                | ImportError::AccountExistsAlready
                | ImportError::ClientUpdateRequired
                | ImportError::UsernamePKMismatch
                | ImportError::CouldNotReachServer
                | ImportError::UnexpectedError(_) => panic!("Wrong error: {:#?}", err),
            },
        }
    }

    #[test]
    fn test_account_public_key_mismatch_import() {
        let bad_account_string = {
            let db1 = test_db();
            let db2 = test_db();
            let account1 = DefaultAccountService::create_account(&db1, &random_username()).unwrap();
            let mut account2 =
                DefaultAccountService::create_account(&db2, &random_username()).unwrap();

            account2.username = account1.username;
            DefaultAccountRepo::insert_account(&db2, &account2).unwrap();
            DefaultAccountService::export_account(&db2).unwrap()
        };

        match import_account(&test_config(), &bad_account_string) {
            Ok(_) => panic!("Should have failed"),
            Err(err) => match err {
                ImportError::UsernamePKMismatch => println!("Test passed!"),
                ImportError::AccountStringCorrupted
                | ImportError::AccountExistsAlready
                | ImportError::ClientUpdateRequired
                | ImportError::AccountDoesNotExist
                | ImportError::CouldNotReachServer
                | ImportError::UnexpectedError(_) => panic! {"Wrong error: {:#?}", err},
            },
        }
    }
}
