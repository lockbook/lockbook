mod integration_test;

#[cfg(test)]
mod server_version_tests {
    use crate::integration_test::{generate_account, test_config};
    use lockbook_core::client::{ApiError, ClientImpl};
    use lockbook_core::model::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};
    use lockbook_core::service::clock_service::ClockImpl;
    use lockbook_core::service::crypto_service::RSAImpl;
    use lockbook_core::{create_account, get_account};
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

        let result: Result<RSAPublicKey, ApiError<GetPublicKeyError>> =
            ClientImpl::<RSAImpl<ClockImpl>>::request(
                &generated_account.api_url,
                &GetPublicKeyRequest {
                    username: String::from(&account.username),
                    client_version: "0.0.0".to_string(),
                },
            )
            .map(|r: GetPublicKeyResponse| r.key);

        match result {
            Ok(_) => {
                panic!("Server should have rejected this due to the version being unsupported")
            }
            Err(err) => match err {
                ApiError::Serialize(_)
                | ApiError::SendFailed(_)
                | ApiError::ReceiveFailed(_)
                | ApiError::Deserialize(_) => {
                    panic!("Server should have rejected this due to the version being unsupported")
                }
                ApiError::Api(err2) => match err2 {
                    GetPublicKeyError::InternalError
                    | GetPublicKeyError::InvalidUsername
                    | GetPublicKeyError::UserNotFound => panic!(
                        "Server should have rejected this due to the version being unsupported"
                    ),
                    GetPublicKeyError::ClientUpdateRequired => println!("test passed"),
                },
            },
        }
    }
}
