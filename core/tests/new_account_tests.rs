mod integration_test;

#[cfg(test)]
mod new_account_tests {
    use lockbook_core::assert_matches;
    use lockbook_core::client;
    use lockbook_core::client::ApiError;
    use lockbook_core::service::test_utils::{generate_account, generate_root_metadata};
    use lockbook_models::api::*;

    #[test]
    fn new_account() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();
    }

    #[test]
    fn new_account_duplicate() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let result = client::request(&account, NewAccountRequest::new(&account, &root));
        assert_matches!(
            result,
            Err(ApiError::<NewAccountError>::Endpoint(
                NewAccountError::PublicKeyTaken
            ))
        );
    }

    #[test]
    fn new_account_duplicate_username() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let mut account2 = generate_account();
        account2.username = account.username;
        let (root2, _) = generate_root_metadata(&account2);
        let result = client::request(&account2, NewAccountRequest::new(&account2, &root2));
        assert_matches!(
            result,
            Err(ApiError::<NewAccountError>::Endpoint(
                NewAccountError::UsernameTaken
            ))
        );
    }

    #[test]
    fn new_account_invalid_username() {
        let mut account = generate_account();
        account.username += " ";
        let (root, _) = generate_root_metadata(&account);

        let result = client::request(&account, NewAccountRequest::new(&account, &root));
        assert_matches!(
            result,
            Err(ApiError::<NewAccountError>::Endpoint(
                NewAccountError::InvalidUsername
            ))
        );
    }
}
