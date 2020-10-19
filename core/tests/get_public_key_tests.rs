mod integration_test;

#[cfg(test)]
mod get_public_key_tests {
    use crate::integration_test::{aes_encrypt, generate_account, rsa_encrypt};
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn get_public_key() {
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();
        let key = account.keys.to_public_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                key.clone(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
                rsa_encrypt::<AESKey>(&key.clone(), &folder_key)
            ),
            Ok(_)
        );

        let key2 = ClientImpl::get_public_key(&account.api_url, &account.username).unwrap();

        assert_eq!(key, key2);
    }

    #[test]
    fn get_public_key_not_found() {
        let account = generate_account();

        assert_matches!(
            ClientImpl::get_public_key(&account.api_url, &account.username),
            Err(ApiError::<GetPublicKeyError>::Api(
                GetPublicKeyError::UserNotFound
            ))
        );
    }
}
