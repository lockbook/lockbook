mod integration_test;

#[cfg(test)]
mod delete_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client;
    use lockbook_core::client::ApiError;
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn delete_document() {
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

        // delete document
        client::request(&account, DeleteDocumentRequest { id: doc.id }).unwrap();
    }

    #[test]
    fn delete_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // delete document that wasn't created
        let (doc, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let result = client::request(&account, DeleteDocumentRequest { id: doc.id });
        assert_matches!(
            result,
            Err(ApiError::<DeleteDocumentError>::Endpoint(
                DeleteDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn delete_document_deleted() {
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

        // delete document
        client::request(&account, DeleteDocumentRequest { id: doc.id }).unwrap();

        // delete document again
        let result = client::request(&account, DeleteDocumentRequest { id: doc.id });
        assert_matches!(
            result,
            Err(ApiError::<DeleteDocumentError>::Endpoint(
                DeleteDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn delete_cannot_delete_root() {
        // new account
        let account = generate_account();
        let (mut root, _root_key) = generate_root_metadata(&account);
        root.metadata_version = client::request(&account, NewAccountRequest::new(&account, &root))
            .unwrap()
            .folder_metadata_version;

        // delete root
        let result = client::request(&account, DeleteFolderRequest { id: root.id });
        assert_matches!(
            result,
            Err(ApiError::<DeleteFolderError>::Endpoint(
                DeleteFolderError::CannotDeleteRoot
            ))
        );
    }
}
