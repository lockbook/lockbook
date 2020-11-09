mod integration_test;

#[cfg(test)]
mod delete_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::DefaultClient;

    #[test]
    fn delete_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document
        DefaultClient::request(
            &account,
            DeleteDocumentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        )
        .unwrap();
    }

    #[test]
    fn delete_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // delete document that wasn't created
        let (doc, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            DeleteDocumentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        )
        .unwrap();
    }

    #[test]
    fn delete_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document
        DefaultClient::request(
            &account,
            DeleteDocumentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        )
        .unwrap();

        // delete document again
        let result = DefaultClient::request(
            &account,
            DeleteDocumentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<DeleteDocumentError>::Api(
                DeleteDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn delete_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document with wrong version
        let result = DefaultClient::request(
            &account,
            DeleteDocumentRequest {
                id: doc.id,
                old_metadata_version: doc.metadata_version - 1,
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<DeleteDocumentError>::Api(
                DeleteDocumentError::EditConflict
            ))
        );
    }
}
