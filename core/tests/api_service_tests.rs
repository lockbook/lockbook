#[cfg(test)]
mod request_common_tests {
    use libsecp256k1::PublicKey;

    use lockbook_crypto::clock_service::{get_time, Timestamp};
    use lockbook_models::api::{
        GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, NewAccountError,
        NewAccountRequest,
    };

    use crate::assert_matches;
    use crate::model::state::temp_config;
    use crate::service::api_service::{request_helper, ApiError};
    use crate::service::db_state_service::get_code_version;
    use crate::service::test_utils;
    use crate::{create_account, get_account};

    static CODE_VERSION: fn() -> &'static str = || "0.0.0";

    #[test]
    fn forced_upgrade() {
        let cfg = temp_config();
        let generated_account = test_utils::generate_account();
        create_account(&cfg, &generated_account.username, &generated_account.api_url).unwrap();
        let account = get_account(&cfg).unwrap();

        let result: Result<PublicKey, ApiError<GetPublicKeyError>> = request_helper(
            &account,
            GetPublicKeyRequest { username: account.username.clone() },
            CODE_VERSION,
            get_time,
        )
        .map(|r: GetPublicKeyResponse| r.key);

        assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ClientUpdateRequired));
    }

    static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(get_time().0 - 3600000);

    #[test]
    fn expired_request() {
        let account = test_utils::generate_account();
        let (root, _) = test_utils::generate_root_metadata(&account);

        let result = request_helper(
            &account,
            NewAccountRequest::new(&account, &root),
            get_code_version,
            EARLY_CLOCK,
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::ExpiredAuth));
    }

    // todo: these are actually integration tests
    // todo: test for invalid signature, signature mismatch during create account request
}
