mod integration_test;

#[cfg(test)]
mod get_document_tests {
    use crate::integration_test::{
        aes_decrypt, aes_encrypt, generate_account, random_filename, rsa_encrypt,
    };
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn get_document() {
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

        // get document
        assert_eq!(
            aes_decrypt(
                &doc_key,
                &ClientImpl::get_document(&account.api_url, doc_id, version)
                    .unwrap(),
            ),
            "doc content".as_bytes()
        );
    }

    #[test]
    fn get_document_not_found() {
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

        // get document we never created
        assert_matches!(
            ClientImpl::get_document(&account.api_url, Uuid::new_v4(), 0,),
            Err(ApiError::<GetDocumentError>::Api(
                GetDocumentError::DocumentNotFound
            ))
        );
    }
}
