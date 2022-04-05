#[cfg(test)]
mod auth_tests {
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::path_service::create_at_path;
    use lockbook_core::service::test_utils::{create_account, test_config};
    use lockbook_core::{assert_matches, get_file_by_path, path, sync_all};
    use lockbook_models::api::*;
    use lockbook_models::crypto::AESEncrypted;
    use lockbook_models::file_metadata::FileMetadataDiff;

    #[test]
    fn upsert_id_takeover() {
        let db1 = &test_config();
        let db2 = &test_config();

        let account1 = create_account(db1).0;
        let (account2, account2_root) = create_account(db2);

        let mut file1 = {
            let path = path!(account1, "test.md");
            let id = create_at_path(db1, path).unwrap().id;
            sync_all(db1, None).unwrap();
            api_service::request(&account1, GetUpdatesRequest { since_metadata_version: 0 })
                .unwrap()
                .file_metadata
                .iter()
                .find(|&f| f.id == id)
                .unwrap()
                .clone()
        };

        file1.parent = account2_root.id;

        // If this succeeded account2 would be able to control file1
        let result = api_service::request(
            &account2,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&file1)] },
        );
        assert_matches!(
            result,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
                FileMetadataUpsertsError::NotPermissioned
            ))
        );
    }

    #[test]
    fn upsert_id_takeover_change_parent() {
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
                .find(|&f| f.id == id)
                .unwrap()
                .clone()
        };

        // If this succeeded account2 would be able to control file1
        let result = api_service::request(
            &account2,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&file1)] },
        );
        assert_matches!(
            result,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
                FileMetadataUpsertsError::NotPermissioned
            ))
        );
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
        assert_matches!(
            result,
            Err(ApiError::<ChangeDocumentContentError>::Endpoint(
                ChangeDocumentContentError::NotPermissioned
            ))
        );
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
        assert_matches!(
            result,
            Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::NotPermissioned))
        );
    }
}
