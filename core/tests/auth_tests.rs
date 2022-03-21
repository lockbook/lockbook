#[cfg(test)]
mod auth_tests {
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::path_service::create_at_path;
    use lockbook_core::service::test_utils::{create_account, test_config};
    use lockbook_core::{assert_matches, get_file_by_path, path, sync_all};
    use lockbook_models::api::*;
    use lockbook_models::crypto::{AESEncrypted, SecretFileName};
    use lockbook_models::file_metadata::FileMetadataDiff;

    #[test]
    fn upsert_impersonate() {
        let db1 = &test_config();
        let db2 = &test_config();

        let account1 = create_account(db1).0;
        let account2 = create_account(db2).0;

        let file1 = {
            let path = path!(account1, "test.md");
            let id = create_at_path(db1, path).unwrap().id;
            sync_all(db1, None).unwrap();
            api_service::request(&account1, GetUpdatesRequest { since_metadata_version: 0 })
                .unwrap()
                .file_metadata
                .iter()
                .filter(|&f| f.id == id)
                .next()
                .unwrap()
                .clone()
        };

        let mut file2 = file1.clone();
        file2.name = SecretFileName {
            encrypted_value: AESEncrypted {
                value: vec![69],
                nonce: vec![69],
                _t: Default::default(),
            },
            hmac: [0; 32],
        };

        let result = api_service::request(
            &account2,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(file1.id, &file1.name, &file2)],
            },
        );
        assert_matches!(result, Err(ApiError::<FileMetadataUpsertsError>::InvalidAuth));
    }

    #[test]
    fn change_document_content() {
        let db1 = &test_config();
        let db2 = &test_config();

        let account1 = create_account(db1).0;
        let account2 = create_account(db2).0;

        let file = {
            let path = path!(account1, "test.md");
            create_at_path(db1, path).unwrap();
            sync_all(db1, None).unwrap();
            get_file_by_path(db1, path).unwrap()
        };

        let result = api_service::request(
            &account2,
            ChangeDocumentContentRequest {
                id: file.id,
                old_metadata_version: file.metadata_version,
                new_content: AESEncrypted {
                    value: vec![69],
                    nonce: vec![69],
                    _t: Default::default(),
                },
            },
        );
        assert_matches!(result, Err(ApiError::<ChangeDocumentContentError>::InvalidAuth));
    }

    #[test]
    fn get_someone_else_document() {
        let db1 = &test_config();
        let db2 = &test_config();

        let account1 = create_account(db1).0;
        let account2 = create_account(db2).0;

        let file = {
            let path = path!(account1, "test.md");
            create_at_path(db1, path).unwrap();
            sync_all(db1, None).unwrap();
            get_file_by_path(db1, path).unwrap()
        };

        let result = api_service::request(&account2, GetDocumentRequest::from(&file));
        assert_matches!(result, Err(ApiError::<GetDocumentError>::InvalidAuth));
    }
}
