use libsecp256k1::PublicKey;

use lockbook_core::get_code_version;
use lockbook_core::service::api_service::{request_helper, ApiError};
use lockbook_crypto::clock_service::{get_time, Timestamp};
use lockbook_models::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};
use test_utils::assert_matches;
use test_utils::test_core_with_account;

static CODE_VERSION: fn() -> &'static str = || "0.0.0";

#[test]
fn forced_upgrade() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

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
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let result = request_helper(
        &account,
        GetPublicKeyRequest { username: account.username.clone() },
        get_code_version,
        EARLY_CLOCK,
    );
    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ExpiredAuth));
}

// todo: test for invalid signature, signature mismatch during create account request
