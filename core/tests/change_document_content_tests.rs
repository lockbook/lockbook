mod integration_test;

#[cfg(test)]
mod change_document_content_tests {
    use crate::integration_test::{
        aes_encrypt, generate_account, random_filename, rsa_encrypt,
    };
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn change_document_content() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
                rsa_encrypt::<AESKey>(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();
        let version = ClientImpl::create_document(
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
        )
        .unwrap();

        // change document content
        assert_matches!(
            ClientImpl::change_document_content(
                &account.api_url,
                &account.username,
                doc_id,
                version,
                aes_encrypt(&doc_key, &String::from("new doc content").into_bytes()),
            ),
            Ok(_)
        );
    }

    #[test]
    fn change_document_content_not_found() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
                rsa_encrypt::<AESKey>(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // change content of document we never created
        assert_matches!(
            ClientImpl::change_document_content(
                &account.api_url,
                &account.username,
                Uuid::new_v4(),
                0,
                aes_encrypt(&folder_key, &String::from("new doc content").into_bytes()),
            ),
            Err(ApiError::<ChangeDocumentContentError>::Api(
                ChangeDocumentContentError::DocumentNotFound
            ))
        );
    }
}
