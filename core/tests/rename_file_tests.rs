#[cfg(test)]
mod rename_document_tests {
    use lockbook_core::assert_get_updates_required;
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{
        generate_account, generate_file_metadata, generate_root_metadata, random_filename,
    };
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn rename_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
        )
        .unwrap();

        // rename document
        let old_name = doc.name.clone();
        doc.name = random_filename();
        api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc)],
            },
        )
        .unwrap();
    }

    #[test]
    fn rename_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                // create document as if renaming an existing document
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
                FileMetadataUpsertsError::NewFileHasOldParentAndName
            ))
        );
    }

    #[test]
    fn rename_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
        )
        .unwrap();

        // rename document
        let old_name = doc.name.clone();
        doc.name = random_filename();
        doc.deleted = true;
        api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc)],
            },
        )
        .unwrap();
    }

    #[test]
    fn rename_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(
            &account,
            FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(&doc)] },
        )
        .unwrap();

        // rename document
        doc.name = random_filename();
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                // use incorrect previous name
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        );
        assert_matches!(result, UPDATES_REQ);
    }

    #[test]
    fn rename_document_path_taken() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create 2 document
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let (doc2, _doc_key2) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc), FileMetadataDiff::new(&doc2)],
            },
        )
        .unwrap();

        // rename document to match name of other document
        let old_name = doc.name.clone();
        doc.name = doc2.name;
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &old_name, &doc)],
            },
        );
        assert_matches!(result, UPDATES_REQ);
    }

    #[test]
    fn rename_folder_cannot_rename_root() {
        // new account
        let account = generate_account();
        let (root, _root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // rename root
        let result = api_service::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &root.name, &root)],
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<FileMetadataUpsertsError>::Endpoint(
                FileMetadataUpsertsError::RootImmutable
            ))
        );
    }
}
