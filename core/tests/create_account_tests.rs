mod integration_test;

#[cfg(test)]
mod change_document_content_tests {
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{generate_account, generate_root_metadata};
    use lockbook_models::api::*;

    #[test]
    fn create_account() {
        // new account
        let account = generate_account();
        let (root, _root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();
    }

    #[test]
    fn create_account_username_case() {
        // new account
        let mut account = generate_account();
        let (mut root, _root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();
        let old_username = account.username.clone();
        account.username = account.username.to_uppercase();
        root.user_access_keys.insert(account.username.to_uppercase(), root.user_access_keys.get(&old_username).unwrap().clone());
        match api_service::request(&account, NewAccountRequest::new(&account, &root)) {
            Err(ApiError::Endpoint(NewAccountError::UsernameTaken)) => {} // Test pass
            _ => panic!("Usernames must be case sensitive")
        }
    }
}
