mod integration_test;

#[cfg(test)]
mod change_document_content_tests {
    use lockbook_core::assert_matches;
    use lockbook_core::service::client;
    use lockbook_core::service::client::ApiError;
    use lockbook_core::service::test_utils::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_crypto::symkey;
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;
    use uuid::Uuid;

    #[test]
    fn change_document_content() {
        // new account
        let account = generate_account();
        let (mut root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // get root metadata version
        root.metadata_version = client::request(
            &account,
            GetUpdatesRequest {
                since_metadata_version: 0,
            },
        )
        .unwrap()
        .file_metadata[0]
            .metadata_version;

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc)],
            },
        )
        .unwrap();

        // get document metadata version
        doc.metadata_version = client::request(
            &account,
            GetUpdatesRequest {
                since_metadata_version: root.metadata_version,
            },
        )
        .unwrap()
        .file_metadata[0]
            .metadata_version;

        // change document content
        client::request(
            &account,
            ChangeDocumentContentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version,
                new_content: aes_encrypt(&doc_key, &String::from("new doc content").into_bytes()),
            },
        )
        .unwrap();
    }

    #[test]
    fn change_document_content_not_found() {
        // new account
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // change content of document we never created
        let result = client::request(
            &account,
            ChangeDocumentContentRequest {
                id: Uuid::new_v4(),
                old_metadata_version: 0,
                new_content: aes_encrypt(
                    &symkey::generate_key(),
                    &String::from("new doc content").into_bytes(),
                ),
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<ChangeDocumentContentError>::Endpoint(
                ChangeDocumentContentError::DocumentNotFound
            ))
        );
    }
}
