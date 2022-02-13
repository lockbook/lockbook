#[cfg(test)]
mod get_document_tests {
    use uuid::Uuid;

    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{
        generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_models::api::*;
    use lockbook_models::crypto::AESEncrypted;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn get_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
        )
        .unwrap();

        // get metadata version
        let metadata_version = api_service::request(
            &account,
            GetUpdatesRequest { since_metadata_version: root.metadata_version },
        )
        .unwrap()
        .file_metadata
        .iter()
        .filter(|&f| f.id == doc.id)
        .next()
        .unwrap()
        .metadata_version;

        // update document content
        api_service::request(
            &account,
            ChangeDocumentContentRequest {
                id: doc.id,
                old_metadata_version: metadata_version,
                new_content: AESEncrypted {
                    value: vec![69],
                    nonce: vec![69],
                    _t: Default::default(),
                },
            },
        )
        .unwrap();

        // get content version
        let content_version = api_service::request(
            &account,
            GetUpdatesRequest { since_metadata_version: metadata_version },
        )
        .unwrap()
        .file_metadata
        .iter()
        .filter(|&f| f.id == doc.id)
        .next()
        .unwrap()
        .content_version;

        // get document
        let result =
            &api_service::request(&account, GetDocumentRequest { id: doc.id, content_version })
                .unwrap();
        assert_eq!(
            result.content,
            Some(AESEncrypted { value: vec!(69), nonce: vec!(69), _t: Default::default() })
        );
    }

    #[test]
    fn get_document_not_found() {
        // new account
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // get document we never created
        let result = api_service::request(
            &account,
            GetDocumentRequest { id: Uuid::new_v4(), content_version: 0 },
        );
        assert_matches!(
            result,
            Err(ApiError::<GetDocumentError>::Endpoint(GetDocumentError::DocumentNotFound))
        );
    }
}
