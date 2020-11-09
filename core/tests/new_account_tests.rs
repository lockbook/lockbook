mod integration_test;

#[cfg(test)]
mod new_account_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_root_metadata, rsa_encrypt,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use lockbook_core::DefaultClient;
    use uuid::Uuid;

    #[test]
    fn new_account() {
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();
    }

    #[test]
    fn new_account_duplicate() {
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        );
        assert_matches!(
            result,
            Err(ApiError::<NewAccountError>::Api(
                NewAccountError::UsernameTaken
            ))
        );
    }

    #[test]
    fn new_account_invalid_username() {
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::new_account(
                &account.api_url,
                &(account.username.clone() + " "),
                account.private_key.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
                rsa_encrypt(&account.private_key.to_public_key(), &folder_key)
            ),
            Err(ApiError::<NewAccountError>::Api(
                NewAccountError::InvalidUsername
            ))
        );
    }

    // #[test]
    // fn new_account_invalid_public_key() {
    //     let account = generate_account();
    //     let folder_id = Uuid::new_v4();
    //     let folder_key = AESImpl::generate_key();

    //     assert_matches!(
    //         DefaultClient::new_account(
    //                 //             &account.username,
    //             &sign(&account),
    //             RSAPrivateKey::from_components(
    //                 BigUint::from_bytes_be(b"a"),
    //                 BigUint::from_bytes_be(b"a"),
    //                 BigUint::from_bytes_be(b"a"),
    //                 vec![
    //                     BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
    //                     BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
    //                 ],
    //             )
    //             .to_public_key(),
    //             folder_id,
    //             FolderAccessInfo {
    //                 folder_id: folder_id,
    //                 access_key: aes_encrypt(&folder_key, &folder_key),
    //             },
    //             rsa_encrypt(&account.private_key.to_public_key(), &folder_key)
    //         ),
    //         Err(ApiError::<NewAccountError>::Api(
    //             NewAccountError::InvalidPublicKey
    //         ))
    //     );
    // }

    // #[test]
    // fn new_account_invalid_signature() {
    //     let account = generate_account();
    //     let folder_id = Uuid::new_v4();
    //     let folder_key = AESImpl::generate_key();

    //     assert_matches!(
    //         DefaultClient::new_account(
    //                 //             &account.username,
    //             &SignedValue {
    //                 content: String::default(),
    //                 signature: String::default(),
    //             },
    //             account.private_key.to_public_key(),
    //             folder_id,
    //             FolderAccessInfo {
    //                 folder_id: folder_id,
    //                 access_key: aes_encrypt(&folder_key, &folder_key),
    //             },
    //             rsa_encrypt(&account.private_key.to_public_key(), &folder_key)
    //         ),
    //         Err(ApiError::<NewAccountError>::Api(NewAccountError::InvalidAuth))
    //     );
    // }
}
