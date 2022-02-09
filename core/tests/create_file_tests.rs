#[cfg(test)]
mod create_document_tests {
    use uuid::Uuid;

    use lockbook_core::assert_get_updates_required;
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{generate_account, generate_file_metadata, generate_root_metadata};
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn create_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] })
            .unwrap();
    }

    #[test]
    fn create_document_duplicate_id() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] })
            .unwrap();

        // create document with same id and key
        let result =
            api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] });
        assert_get_updates_required!(result);
    }

    #[test]
    fn create_document_duplicate_path() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] })
            .unwrap();

        // create document with same path
        let (mut doc2, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc2.name = doc.name;
        let result =
            api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc2)] });

        assert_get_updates_required!(result);
    }

    #[test]
    fn create_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.parent = Uuid::new_v4();
        let result =
            api_service::request(&account, FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] });

        assert_get_updates_required!(result);
    }
}
