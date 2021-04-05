mod integration_test;

#[cfg(test)]
mod get_updates_test {
    use crate::integration_test::{generate_account, generate_root_metadata};
    use lockbook_core::client::Client;
    use lockbook_core::DefaultClient;
    use lockbook_models::api::{GetUpdatesRequest, NewAccountRequest};

    #[test]
    fn get_updates() {
        // new account
        let account = generate_account();
        let (mut root, _) = generate_root_metadata(&account);
        root.metadata_version =
            DefaultClient::request(&account, NewAccountRequest::new(&account, &root))
                .unwrap()
                .folder_metadata_version;

        // get updates at version 0
        let result = DefaultClient::request(
            &account,
            GetUpdatesRequest {
                since_metadata_version: 0,
            },
        )
        .unwrap()
        .file_metadata
        .len();
        assert_eq!(result, 1);

        // get updates at version of root folder
        let result = DefaultClient::request(
            &account,
            GetUpdatesRequest {
                since_metadata_version: root.metadata_version,
            },
        )
        .unwrap()
        .file_metadata
        .len();
        assert_eq!(result, 0);
    }
}
