mod integration_test;

#[cfg(test)]
mod get_updates_test {
    use crate::integration_test::{aes_encrypt, generate_account, rsa_encrypt};
    use lockbook_core::client::{Client, ClientImpl};
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::clock_service::ClockImpl;
    use lockbook_core::service::crypto_service::{AESImpl, RSAImpl, SymmetricCryptoService};
    use uuid::Uuid;

    #[test]
    fn get_updates() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();

        let version = ClientImpl::<RSAImpl<ClockImpl>>::new_account(
            &account.api_url,
            &account.username,
            account.private_key.to_public_key(),
            folder_id,
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_encrypt(&folder_key, &folder_key),
            },
            rsa_encrypt(&account.private_key.to_public_key(), &folder_key),
        )
        .unwrap();

        // get updates at version 0
        assert_eq!(
            ClientImpl::<RSAImpl::<ClockImpl>>::get_updates(&account.api_url, &account.username, 0,)
                .unwrap()
                .len(),
            1
        );

        // get updates at version of root folder
        assert_eq!(
            ClientImpl::<RSAImpl::<ClockImpl>>::get_updates(
                &account.api_url,
                &account.username,
                version,
            )
            .unwrap()
            .len(),
            0
        );
    }
}
