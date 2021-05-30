mod integration_test;

#[cfg(test)]
mod get_document_tests {
    use lockbook_core::assert_matches;
    use lockbook_core::client;
    use lockbook_core::client::ApiError;
    use lockbook_core::service::test_utils::{
        aes_decrypt, aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileType;
    use uuid::Uuid;

    #[test]
    fn get_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.content_version = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // get document
        let result = aes_decrypt(
            &doc_key,
            &client::request(
                &account,
                GetDocumentRequest {
                    id: doc.id,
                    content_version: doc.content_version,
                },
            )
            .unwrap()
            .content,
        );
        assert_eq!(result, String::from("doc content").into_bytes());
    }

    #[test]
    fn get_document_not_found() {
        // new account
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // get document we never created
        let result = client::request(
            &account,
            GetDocumentRequest {
                id: Uuid::new_v4(),
                content_version: 0,
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<GetDocumentError>::Endpoint(
                GetDocumentError::DocumentNotFound
            ))
        );
    }
}
