mod integration_test;

#[cfg(test)]
mod delete_document_tests {
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
    fn delete_document() {
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
        )
        .unwrap();

        // delete document
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
            ),
            Ok(_)
        );
    }

    #[test]
    fn delete_document_not_found() {
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

        // delete document that wasn't created
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
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
        )
        .unwrap();

        // delete document
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
            ),
            Ok(_)
        );

        // delete document again
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
            ),
            Err(ApiError::<DeleteDocumentError>::Api(
                DeleteDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn delete_cannot_delete_root() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();
        let version = ClientImpl::new_account(
            &account.api_url,
            &account.username,
            &sign(&account),
            account.keys.to_public_key(),
            folder_id,
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &folder_key),
            },
            rsa_key(&account.keys.to_public_key(), &folder_key),
        )
            .unwrap();

        // rename document
        assert_matches!(
            ClientImpl::delete_folder(
                &account.api_url,
                &account.username,
                &sign(&account),
                folder_id,
            ),
            Err(ApiError::<DeleteFolderError>::Api(
                DeleteFolderError::CannotDeleteRoot
            ))
        );
    }
}
