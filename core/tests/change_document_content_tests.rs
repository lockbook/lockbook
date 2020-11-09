mod integration_test;

#[cfg(test)]
mod change_document_content_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use lockbook_core::DefaultClient;
    use uuid::Uuid;

    #[test]
    fn change_document_content() {
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

        // change document content
        DefaultClient::request(
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
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // change content of document we never created
        let result = DefaultClient::request(
            &account,
            ChangeDocumentContentRequest {
                id: Uuid::new_v4(),
                old_metadata_version: 0,
                new_content: aes_encrypt(
                    &AESImpl::generate_key(),
                    &String::from("new doc content").into_bytes(),
                ),
            },
        );
        assert_matches!(
            result,
            Err(ApiError::<ChangeDocumentContentError>::Api(
                ChangeDocumentContentError::DocumentNotFound
            ))
        );
    }
}
