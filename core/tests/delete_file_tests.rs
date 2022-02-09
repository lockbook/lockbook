#[cfg(test)]
mod delete_document_tests {
    use lockbook_core::assert_get_updates_required;
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{generate_account, generate_file_metadata, generate_root_metadata};
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn delete_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] })
            .unwrap();

        // delete document
        doc.deleted = true;
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)] },
        )
        .unwrap();
    }

    #[test]
    fn delete_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                // create document as if deleting an existing document
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn delete_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] })
            .unwrap();

        // delete document
        doc.deleted = true;
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)] },
        )
        .unwrap();

        // delete document again
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)] },
        )
        .unwrap();
    }

    #[test]
    fn delete_cannot_delete_root() {
        // new account
        let account = generate_account();
        let (mut root, _root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // delete root
        root.deleted = true;
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                // create document as if deleting an existing document
                updates: vec![FileMetadataDiff::new_diff(root.id, &root.name, &root)],
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(FileMetadataUpsertsError::RootImmutable))
        );
    }
}
