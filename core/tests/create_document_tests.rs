mod integration_test;

#[cfg(test)]
mod create_document_tests {
    use crate::integration_test::{
        aes_key, aes_str, generate_account, random_filename, rsa_key, sign,
    };
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AesImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn create_document() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &random_filename(),
                folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );
    }

    #[test]
    fn create_document_duplicate_id() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &random_filename(),
                folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        // create document with same id and key
        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &random_filename(),
                folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
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
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let doc_name = random_filename();

        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &doc_name,
                folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        // create document with same path
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &doc_name,
                folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
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
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        let parent_folder_id = Uuid::new_v4();

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let doc_name = random_filename();

        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                &doc_name,
                parent_folder_id,
                aes_str(&doc_key, "doc content"),
                FolderAccessInfo {
                    folder_id: parent_folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Err(ApiError::<CreateDocumentError>::Api(
                CreateDocumentError::ParentNotFound
            ))
        );
    }
}
