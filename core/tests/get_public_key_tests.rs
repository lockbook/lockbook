mod integration_test;

#[cfg(test)]
mod get_public_key_tests {
    use crate::assert_matches;
    use crate::integration_test::{generate_account, generate_root_metadata};
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::DefaultClient;

    #[test]
    fn get_public_key() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let result = DefaultClient::request(&account, GetPublicKeyRequest {})
            .unwrap()
            .key;
        assert_eq!(result, account.private_key.to_public_key());
    }

    #[test]
    fn get_public_key_not_found() {
        let account = generate_account();

        let result = DefaultClient::request(&account, GetPublicKeyRequest {});
        assert_matches!(
            result,
            Err(ApiError::<GetPublicKeyError>::Endpoint(
                GetPublicKeyError::UserNotFound
            ))
        );
    }
}
