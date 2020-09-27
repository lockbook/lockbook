mod integration_test;

#[cfg(test)]
mod get_usage_tests {
    use crate::integration_test::{
        aes_key, aes_str, generate_account, random_filename, rsa_key, sign,
    };
    use lockbook_core::client::{Client, ClientImpl};
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AesImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn get_usage() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
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
        let doc_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_document(
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
                &random_filename(),
                folder_id,
                aes_str(&doc_key, "0000000000000000"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        let usage_a = ClientImpl::get_usage(&account.username).unwrap().usage;

        assert_matches!(
            ClientImpl::create_document(
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
                &random_filename(),
                folder_id,
                aes_str(&doc_key, "00000000000000000000000000000000"),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        let usage_b = ClientImpl::get_usage(&account.username).unwrap().usage;

        assert_matches!(
            ClientImpl::create_document(
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
                &random_filename(),
                folder_id,
                aes_str(
                    &doc_key,
                    "0000000000000000000000000000000000000000000000000000000000000000"
                ),
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &doc_key),
                },
            ),
            Ok(_)
        );

        let usage_c = ClientImpl::get_usage(&account.username).unwrap().usage;

        let usage_tot = usage_a + usage_b + usage_c;

        assert_matches!(
            ClientImpl::get_usage(&account.username).unwrap().usage,
            usage_tot
        );
    }
}
