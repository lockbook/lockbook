mod integration_test;

#[cfg(test)]
mod server_version_tests {
    use crate::assert_matches;
    use crate::integration_test::{generate_account, test_config};
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};
    use lockbook_core::{create_account, get_account, DefaultClient};
    use rsa::RSAPublicKey;

    #[test]
    fn forced_upgrade() {
        let cfg = test_config();
        let generated_account = generate_account();
        create_account(
            &cfg,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let account = get_account(&cfg).unwrap();

        let result: Result<RSAPublicKey, ApiError<GetPublicKeyError>> = DefaultClient::request(
            &account,
            GetPublicKeyRequest {
                username: account.username.clone(),
            },
        )
        .map(|r: GetPublicKeyResponse| r.key);

        assert_matches!(
            result,
            Err(ApiError::<GetPublicKeyError>::ClientUpdateRequired)
        );
    }
}
