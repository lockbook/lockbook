mod integration_test;

#[cfg(test)]
mod delete_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::service::clock_service::ClockImpl;
    use lockbook_core::service::code_version_service::CodeVersionImpl;
    use lockbook_core::service::crypto_service::RSAImpl;
    use uuid::Uuid;

    #[test]
    fn delete_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // delete document
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            &DeleteDocumentRequest {
                username: account.username.clone(),
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
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // delete document that wasn't created
        assert_matches!(
            DefaultClient::delete_document(
                &account.api_url,
                &account.username,
                Uuid::new_v4(),
                0,
            ),
            Err(ApiError::<DeleteDocumentError>::Api(
                DeleteDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn delete_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // delete document
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            &DeleteDocumentRequest {
                username: account.username.clone(),
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        )
        .unwrap();

        // delete document again
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            &DeleteDocumentRequest {
                username: account.username.clone(),
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
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // delete document with wrong version
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            &DeleteDocumentRequest {
                username: account.username.clone(),
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
