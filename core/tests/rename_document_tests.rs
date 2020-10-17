mod integration_test;

#[cfg(test)]
mod rename_document_tests {
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
    fn rename_document() {
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
        let version = ClientImpl::create_document(
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
        )
        .unwrap();

        // rename document
        assert_matches!(
            ClientImpl::rename_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                &random_filename(),
            ),
            Ok(_)
        );
    }

    #[test]
    fn rename_document_not_found() {
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

        // rename document that wasn't created
        assert_matches!(
            ClientImpl::rename_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
                0,
                &random_filename(),
            ),
            Err(ApiError::<RenameDocumentError>::Api(
                RenameDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn rename_document_deleted() {
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
        let version = ClientImpl::create_document(
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
        )
        .unwrap();

        // delete document
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
            ),
            Ok(_)
        );

        // rename deleted document
        assert_matches!(
            ClientImpl::rename_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                &random_filename(),
            ),
            Err(ApiError::<RenameDocumentError>::Api(
                RenameDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn rename_document_conflict() {
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
        let version = ClientImpl::create_document(
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
        )
        .unwrap();

        // rename document
        assert_matches!(
            ClientImpl::rename_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version - 1,
                &random_filename(),
            ),
            Err(ApiError::<RenameDocumentError>::Api(
                RenameDocumentError::EditConflict
            ))
        );
    }

    #[test]
    fn rename_document_path_taken() {
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
        let version = ClientImpl::create_document(
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
        )
        .unwrap();

        // create document in same folder
        let doc_id2 = Uuid::new_v4();
        let doc_key2 = AesImpl::generate_key();
        let doc_name2 = random_filename();
        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id2,
                &doc_name2,
                folder_id,
                aes_str(&doc_key2, "doc content"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key2),
                },
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            ClientImpl::rename_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                &doc_name2,
            ),
            Err(ApiError::<RenameDocumentError>::Api(
                RenameDocumentError::DocumentPathTaken
            ))
        );
    }
}
