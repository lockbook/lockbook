mod integration_test;

#[cfg(test)]
mod request_common_tests {
    use crate::assert_matches;
    use crate::integration_test::{generate_account, generate_root_metadata, test_config};
    use libsecp256k1::{Message, PublicKey, SecretKey};
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::service::code_version_service::CodeVersion;
    use lockbook_core::{
        create_account, get_account, DefaultClock, DefaultCodeVersion, DefaultPKCrypto,
    };
    use lockbook_crypto::clock_service::{Clock, ClockImpl};
    use lockbook_crypto::pubkey::{
        ECSignError, ECVerifyError, ElipticCurve, GetAesKeyError, PubKeyCryptoService,
    };
    use lockbook_models::api::{
        GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, NewAccountError,
        NewAccountRequest,
    };
    use lockbook_models::crypto::ECSigned;

    use serde::Serialize;
    use sha2::{Digest, Sha256};

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

        let result: Result<PublicKey, ApiError<GetPublicKeyError>> =
            ClientImpl::<DefaultPKCrypto, MockCodeVersion>::request(
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
        let result = ClientImpl::<ElipticCurve<MockClock>, DefaultCodeVersion>::request(
            &account,
            NewAccountRequest::new(&account, &root),
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::ExpiredAuth));
    }

    struct MockEC;
    impl PubKeyCryptoService for MockEC {
        fn generate_key() -> SecretKey {
            unimplemented!()
        }

        fn sign<T: Serialize>(sk: &SecretKey, to_sign: T) -> Result<ECSigned<T>, ECSignError> {
            let timestamped = DefaultClock::timestamp(to_sign);
            let serialized =
                bincode::serialize(&timestamped).map_err(ECSignError::Serialization)?;
            let digest = Sha256::digest(&serialized);
            let message = &Message::parse_slice(&digest).map_err(ECSignError::ParseError)?;
            let (signature, _) = libsecp256k1::sign(&message, &sk);
            Ok(ECSigned {
                timestamped_value: timestamped,
                signature: signature.serialize()[0..31].to_vec(),
                public_key: PublicKey::from_secret_key(&sk),
            })
        }

        fn verify<T: Serialize>(
            _pk: &PublicKey,
            _signed: &ECSigned<T>,
            _max_delay_ms: u64,
            _max_skew_ms: u64,
        ) -> Result<(), ECVerifyError> {
            unimplemented!()
        }

        fn get_aes_key(_: &SecretKey, _: &PublicKey) -> Result<[u8; 32], GetAesKeyError> {
            unimplemented!()
        }
    }

    #[test]
    fn bad_signature() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        let result = ClientImpl::<MockEC, DefaultCodeVersion>::request(
            &account,
            NewAccountRequest::new(&account, &root),
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::InvalidAuth));
    }
}
