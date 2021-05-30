mod integration_test;

#[cfg(test)]
mod create_document_tests {
    use lockbook_core::assert_matches;
    use lockbook_core::client;
    use lockbook_core::client::ApiError;
    use lockbook_core::service::test_utils::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileType;
    use uuid::Uuid;

    #[test]
    fn create_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();
    }

    #[test]
    fn create_document_duplicate_id() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create document with same id and key
        let result = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        );
        assert_matches!(
            result,
            Err(ApiError::<CreateDocumentError>::Endpoint(
                CreateDocumentError::FileIdTaken
            ))
        );
    }

    #[test]
    fn create_document_duplicate_path() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create document with same path
        let (mut doc2, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc2.name = doc.name;
        let result = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc2,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        );

        assert_matches!(
            result,
            Err(ApiError::<CreateDocumentError>::Endpoint(
                CreateDocumentError::DocumentPathTaken
            ))
        );
    }

    #[test]
    fn create_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.parent = Uuid::new_v4();
        let result = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        );

        assert_matches!(
            result,
            Err(ApiError::<CreateDocumentError>::Endpoint(
                CreateDocumentError::ParentNotFound
            ))
        );
    }
}
