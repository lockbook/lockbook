mod test_utils;

#[cfg(test)]
mod auth_tests {
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_models::api::*;
    use lockbook_models::crypto::AESEncrypted;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use crate::test_utils::{test_core_with_account, path};
    use crate::assert_matches;

    #[test]
    fn upsert_id_takeover() {
        let core1 = test_core_with_account();
        let core2 = test_core_with_account();

        let mut file1 = {
            let path = &path(&core1, "test.md");
            let id = core1.create_at_path(path).unwrap().id;
            core1.sync(None).unwrap();
            api_service::request(&core1.get_account().unwrap(), GetUpdatesRequest { since_metadata_version: 0 })
                .unwrap()
                .file_metadata
                .iter()
                .find(|&f| f.id == id)
                .unwrap()
                .clone()
        };

        file1.parent = core2.get_root().unwrap().id;

        // If this succeeded account2 would be able to control file1
        let result = api_service::request(
            &core2.get_account().unwrap(),
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
        let core1 = test_core_with_account();
        let core2 = test_core_with_account();
        let account1 = core1.get_account().unwrap();
        let account2 = core2.get_account().unwrap();

        let file1 = {
            let path = &path(&core1, "test.md");
            let id = core1.create_at_path(path).unwrap().id;
            core1.sync(None).unwrap();
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
        let core1 = test_core_with_account();
        let core2 = test_core_with_account();

        let file = {
            let path = &path(&core1, "test.md");
            core1.create_at_path(path).unwrap();
            core1.sync(None).unwrap();
            core1.get_by_path(path).unwrap()
        };

        let result = api_service::request(
            &core2.get_account().unwrap(),
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
        let core1 = test_core_with_account();
        let core2 = test_core_with_account();

        let file = {
            let path = &path(&core1, "test.md");
            core1.create_at_path(path).unwrap();
            core1.sync(None).unwrap();
            core1.get_by_path(path).unwrap()
        };

        let result = api_service::request(&core2.get_account().unwrap(), GetDocumentRequest::from(&file));
        assert_matches!(
            result,
            Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::NotPermissioned))
        );
    }
}
