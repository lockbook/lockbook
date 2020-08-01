#[cfg(test)]
mod account_tests {
    use lockbook_core::client::Error;
    use lockbook_core::model::api::NewAccountError::UsernameTaken;
    use lockbook_core::service::account_service::{AccountCreationError, AccountService, AccountImportError};
    use lockbook_core::{
        DefaultAccountRepo, DefaultAccountService, DefaultFileMetadataRepo, DefaultSyncService,
    };

    use crate::{random_username, test_db};
    use lockbook_core::model::api::NewAccountError;
    use lockbook_core::repo::account_repo::AccountRepo;
    use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
    use lockbook_core::service::sync_service::SyncService;
    use lockbook_core::model::account::Account;
    use rsa::{RSAPrivateKey, BigUint};
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

        match DefaultAccountService::create_account(&db2, username).unwrap_err() {
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

        DefaultAccountDb::insert_account(&db1, &account).unwrap();

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
    fn test_import_export_opposites() {
        let account_string = "BgAAAAAAAABwYXJ0aDRAAAAAAAAAAJnSeo+j1kZ6zi/Sfw/k6h8adzTImXng3ZXqvSKOUyYatb1Xm7Kh3AFPNSkTytGC/3ajran8/WhUnImJobEg0MGQoXdLiuwxtMs45RhuSDlBPPwW+Dw8EUt3ElEkgMkXXsZzcIfOSuTxTh+pmJWJJO5v4tyTu0jhXP7WJ9yK44EzQUpWVwTLb4t81wuUU5tJ/f4ybr/UrRmjXSLqKybUdjRQseF4l+aH8Ony3yC93UhlNlZtInoJIZCa+xuoJQsPHM+lzdZcHi3GhAw3t8BSnP5oW/j+mnRbb/h67RRqb+C+7b+x4ixrliCO0ekEhC/W0VhymZQh0YYMb7X/Vm6nSLoBAAAAAAAAAAEAAQBAAAAAAAAAAI1X0y8br/ltxnEYZxfO/6TLorOKEJd5H/0XeDXDiMjSvSPOCzuCbhSGWQVPdU9iegHdCHOrqA21pcSfJ5c2+0I38HRpWYZeQk2ochDTqqe23WJ27kt5CgrK6gXG5MeROCSEMSiJwcelhkdVYf5bSsdqGi681T4416lravO07oSTggy/dw/+w/BcYWXEjN07ujYgt4zOkYBQ4C1t3bVRAjEnx6EkF4UOHxlcbIbdfD/Txmm9AAhIz9MxQLq25U57bK5hoK6orOxxUMIZnpqvy9TH2+AZD2l9HjylVN2wC6gXLfIrPk0NUroxXVRcYuPhkCkvoWtq5bdW++1j5bRxAF4CAAAAAAAAACAAAAAAAAAAhx6QHKVxtuz2yNfzPOb5fJWZmuRWyDFzyrOQXFK7Q3o30iDtP+6AaQuRFX/75N6PDFJfjE/kHobsLd+yhNkDg19EkFM4dceKoR9WylGb3S2QmD9J7ew63EnPMs+mHqBqv1bsgh8+eTwo8teqA0oFSMz0OzwGRz0xn5jzmwZxKcwgAAAAAAAAAN+t+ahUxaKA8d5UDLjzjnxheC/QuneQAJVYDxExP+/9uchnBt1rxYiqBHWgaFiIHgAyfkaak4oFNZ+Cnf/Gb0qjHWGiF/f8/63rmv54XmfbpMifUNYnUSBSbEGU8KNRw1BZpofmadY6KfDV/aoyBUSX7yU9rPT9hbkpjR5oIpXp".to_string();
        let db = test_db();

        DefaultAccountService::import_account(&db, &account_string).unwrap();
        assert_eq!(
            DefaultAccountService::export_account(&db).unwrap(),
            account_string
        );
    }

    #[test]
    fn test_importing_an_account_when_one_exists() {
        let account_string = "BgAAAAAAAABwYXJ0aDRAAAAAAAAAAJnSeo+j1kZ6zi/Sfw/k6h8adzTImXng3ZXqvSKOUyYatb1Xm7Kh3AFPNSkTytGC/3ajran8/WhUnImJobEg0MGQoXdLiuwxtMs45RhuSDlBPPwW+Dw8EUt3ElEkgMkXXsZzcIfOSuTxTh+pmJWJJO5v4tyTu0jhXP7WJ9yK44EzQUpWVwTLb4t81wuUU5tJ/f4ybr/UrRmjXSLqKybUdjRQseF4l+aH8Ony3yC93UhlNlZtInoJIZCa+xuoJQsPHM+lzdZcHi3GhAw3t8BSnP5oW/j+mnRbb/h67RRqb+C+7b+x4ixrliCO0ekEhC/W0VhymZQh0YYMb7X/Vm6nSLoBAAAAAAAAAAEAAQBAAAAAAAAAAI1X0y8br/ltxnEYZxfO/6TLorOKEJd5H/0XeDXDiMjSvSPOCzuCbhSGWQVPdU9iegHdCHOrqA21pcSfJ5c2+0I38HRpWYZeQk2ochDTqqe23WJ27kt5CgrK6gXG5MeROCSEMSiJwcelhkdVYf5bSsdqGi681T4416lravO07oSTggy/dw/+w/BcYWXEjN07ujYgt4zOkYBQ4C1t3bVRAjEnx6EkF4UOHxlcbIbdfD/Txmm9AAhIz9MxQLq25U57bK5hoK6orOxxUMIZnpqvy9TH2+AZD2l9HjylVN2wC6gXLfIrPk0NUroxXVRcYuPhkCkvoWtq5bdW++1j5bRxAF4CAAAAAAAAACAAAAAAAAAAhx6QHKVxtuz2yNfzPOb5fJWZmuRWyDFzyrOQXFK7Q3o30iDtP+6AaQuRFX/75N6PDFJfjE/kHobsLd+yhNkDg19EkFM4dceKoR9WylGb3S2QmD9J7ew63EnPMs+mHqBqv1bsgh8+eTwo8teqA0oFSMz0OzwGRz0xn5jzmwZxKcwgAAAAAAAAAN+t+ahUxaKA8d5UDLjzjnxheC/QuneQAJVYDxExP+/9uchnBt1rxYiqBHWgaFiIHgAyfkaak4oFNZ+Cnf/Gb0qjHWGiF/f8/63rmv54XmfbpMifUNYnUSBSbEGU8KNRw1BZpofmadY6KfDV/aoyBUSX7yU9rPT9hbkpjR5oIpXp".to_string();
        let db = test_db();

        DefaultAccountService::import_account(&db, &account_string).unwrap();
        match DefaultAccountService::import_account(&db, &account_string) {
            Ok(_) => {
                panic!("You should not have been able to import an account if one exists already")
            }
            Err(err) => match err {
                AccountImportError::AccountExistsAlready => println!("Success."),
                _ => panic!("The wrong type of error was returned: {:#?}", err),
            },
        }
    }
}
