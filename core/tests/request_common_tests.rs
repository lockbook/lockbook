mod integration_test;

#[cfg(test)]
mod request_common_tests {
    use crate::assert_matches;
    use crate::integration_test::{generate_account, generate_root_metadata, test_config};
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::service::clock_service::{Clock, ClockImpl};
    use lockbook_core::service::code_version_service::CodeVersion;
    use lockbook_core::service::crypto_service::{
        PubKeyCryptoService, RSADecryptError, RSAEncryptError, RSAImpl, RSASignError,
        RSAVerifyError,
    };
    use lockbook_core::{create_account, get_account, DefaultCodeVersion, DefaultCrypto};
    use lockbook_models::api::{
        GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, NewAccountError,
        NewAccountRequest,
    };
    use lockbook_models::crypto::{RSAEncrypted, RSASigned};
    use rsa::errors::Error;
    use rsa::{RSAPrivateKey, RSAPublicKey};
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    struct MockCodeVersion;
    impl CodeVersion for MockCodeVersion {
        fn get_code_version() -> &'static str {
            "0.0.0"
        }
    }

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
            ClientImpl::<DefaultCrypto, MockCodeVersion>::request(
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

    struct MockClock;
    impl Clock for MockClock {
        fn get_time() -> i64 {
            ClockImpl::get_time() - 3600000
        }
    }

    #[test]
    fn expired_request() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        let result = ClientImpl::<RSAImpl<MockClock>, DefaultCodeVersion>::request(
            &account,
            NewAccountRequest::new(&account, &root),
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::ExpiredAuth));
    }

    struct MockRSA;
    impl PubKeyCryptoService for MockRSA {
        fn generate_key() -> Result<RSAPrivateKey, Error> {
            unimplemented!()
        }

        fn encrypt<T: Serialize + DeserializeOwned>(
            _: &RSAPublicKey,
            _: &T,
        ) -> Result<RSAEncrypted<T>, RSAEncryptError> {
            unimplemented!()
        }

        fn decrypt<T: DeserializeOwned>(
            _: &RSAPrivateKey,
            _: &RSAEncrypted<T>,
        ) -> Result<T, RSADecryptError> {
            unimplemented!()
        }

        fn sign<T: Serialize>(
            private_key: &RSAPrivateKey,
            to_sign: T,
        ) -> Result<RSASigned<T>, RSASignError> {
            let mut result = RSAImpl::<ClockImpl>::sign(private_key, to_sign).unwrap();
            result.signature.pop();
            Ok(result)
        }

        fn verify<T: Serialize>(
            _: &RSAPublicKey,
            _: &RSASigned<T>,
            _: u64,
            _: u64,
        ) -> Result<(), RSAVerifyError> {
            unimplemented!()
        }
    }

    #[test]
    fn bad_signature() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        let result = ClientImpl::<MockRSA, DefaultCodeVersion>::request(
            &account,
            NewAccountRequest::new(&account, &root),
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::InvalidAuth));
    }
}
