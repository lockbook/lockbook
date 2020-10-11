mod integration_test;

#[cfg(test)]
mod new_account_tests {
    use crate::integration_test::{aes_key, generate_account, rsa_key, sign};
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AesImpl, SymmetricCryptoService};
    // use rsa::{BigUint, RSAPrivateKey};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn new_account() {
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
    }

    #[test]
    fn new_account_duplicate() {
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
            Err(ApiError::<NewAccountError>::Api(
                NewAccountError::UsernameTaken
            ))
        );
    }

    #[test]
    fn new_account_invalid_username() {
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &(account.username.clone() + " "),
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
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
    //     let folder_key = AesImpl::generate_key();

    //     assert_matches!(
    //         ClientImpl::new_account(
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
    //                 access_key: aes_key(&folder_key, &folder_key),
    //             },
    //             rsa_key(&account.keys.to_public_key(), &folder_key)
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
    //     let folder_key = AesImpl::generate_key();

    //     assert_matches!(
    //         ClientImpl::new_account(
    //                 //             &account.username,
    //             &SignedValue {
    //                 content: String::default(),
    //                 signature: String::default(),
    //             },
    //             account.keys.to_public_key(),
    //             folder_id,
    //             FolderAccessInfo {
    //                 folder_id: folder_id,
    //                 access_key: aes_key(&folder_key, &folder_key),
    //             },
    //             rsa_key(&account.keys.to_public_key(), &folder_key)
    //         ),
    //         Err(ApiError::<NewAccountError>::Api(NewAccountError::InvalidAuth))
    //     );
    // }
}
