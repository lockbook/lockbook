mod integration_test;

#[cfg(test)]
mod get_updates_test {
    use crate::integration_test::{aes_key, generate_account, rsa_key, sign};
    use lockbook_core::client::{Client, ClientImpl};
    use lockbook_core::service::crypto_service::{AesImpl, SymmetricCryptoService};
    use lockbook_models::crypto::*;
    use uuid::Uuid;

    #[test]
    fn get_updates() {
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

        // get updates at version 0
        assert_eq!(
            ClientImpl::get_updates(&account.api_url, &account.username, &sign(&account), 0,)
                .unwrap()
                .len(),
            1
        );

        // get updates at version of root folder
        assert_eq!(
            ClientImpl::get_updates(
                &account.api_url,
                &account.username,
                &sign(&account),
                version,
            )
            .unwrap()
            .len(),
            0
        );
    }
}
