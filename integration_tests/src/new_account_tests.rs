#[cfg(test)]
mod new_account_tests {
    use lockbook_core::model::crypto::*;
    use crate::{api_loc, generate_account, sign, aes, rsa};
    use lockbook_core::client::{Client, ClientImpl};
    use uuid::Uuid;
    use lockbook_core::service::crypto_service::{
        SymmetricCryptoService, AesImpl
    };

    #[test]
    fn new_account() {
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        ClientImpl::new_account(
            &api_loc(),
            &account.username,
            &sign(&account),
            account.keys.to_public_key(),
            folder_id,
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes(&folder_key, &folder_key),
            },
            rsa(&account.keys.to_public_key(), &folder_key)
        ).unwrap();
    }
}
