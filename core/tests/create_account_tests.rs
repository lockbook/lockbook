mod integration_test;

#[cfg(test)]
mod change_document_content_tests {
    use lockbook_core::client;
    use lockbook_core::service::test_utils::{generate_account, generate_root_metadata};
    use lockbook_models::api::*;

    #[test]
    fn create_account() {
        // new account
        let account = generate_account();
        let (root, _root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();
    }
}
