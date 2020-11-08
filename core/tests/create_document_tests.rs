mod integration_test;

#[cfg(test)]
mod create_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{aes_encrypt, generate_account, random_filename, rsa_encrypt};
    use lockbook_core::client::{ApiError, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::clock_service::ClockImpl;
    use lockbook_core::service::code_version_service::CodeVersionImpl;
    use lockbook_core::service::crypto_service::RSAImpl;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use uuid::Uuid;

    #[test]
    fn create_document() {
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
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &random_filename(),
                folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );
    }

    #[test]
    fn create_document_duplicate_id() {
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
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &random_filename(),
                folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        // create document with same id and key
        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &random_filename(),
                folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Err(ApiError::<CreateDocumentError>::Api(
                CreateDocumentError::FileIdTaken
            ))
        );
    }

    #[test]
    fn create_document_duplicate_path() {
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
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();
        let doc_name = random_filename();

        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &doc_name,
                folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        // create document with same path
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &doc_name,
                folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Err(ApiError::<CreateDocumentError>::Api(
                CreateDocumentError::DocumentPathTaken
            ))
        );
    }

    #[test]
    fn create_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        let parent_folder_id = Uuid::new_v4();

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();
        let doc_name = random_filename();

        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id,
                &doc_name,
                parent_folder_id,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: parent_folder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key),
                },
            ),
            Err(ApiError::<CreateDocumentError>::Api(
                CreateDocumentError::ParentNotFound
            ))
        );
    }
}
