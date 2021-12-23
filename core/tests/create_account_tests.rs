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
        let (root, _root_key) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();
        let old_username  = account.username;
        let mut account = generate_account();
        account.username = old_username.to_uppercase();
        let (root, _root_key) = generate_root_metadata(&account);
        let operation = api_service::request(&account, NewAccountRequest::new(&account, &root));
        match operation {
            Err(ApiError::Endpoint(NewAccountError::UsernameTaken)) => {} // Test pass
            _ => panic!("Usernames must be case sensitive {:?}", operation)
        }
    }
}
