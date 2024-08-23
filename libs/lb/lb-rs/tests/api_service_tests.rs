use libsecp256k1::PublicKey;

use lb_rs::get_code_version;
use lb_rs::logic::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};
use lb_rs::logic::clock::{get_time, Timestamp};
use lb_rs::service::api_service::{ApiError, NetworkOld, Requester};
use test_utils::assert_matches;
use test_utils::test_core_with_account;

static CODE_VERSION: fn() -> &'static str = || "0.0.0";

#[test]
fn forced_upgrade() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let client =
        NetworkOld { client: Default::default(), get_code_version: CODE_VERSION, get_time };

    let result: Result<PublicKey, ApiError<GetPublicKeyError>> = client
        .request(&account, GetPublicKeyRequest { username: account.username.clone() })
        .map(|r: GetPublicKeyResponse| r.key);

    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ClientUpdateRequired));
}

static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(get_time().0 - 3600000);

#[test]
fn expired_request() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let client = NetworkOld { client: Default::default(), get_code_version, get_time: EARLY_CLOCK };

    let result =
        client.request(&account, GetPublicKeyRequest { username: account.username.clone() });
    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ExpiredAuth));
}

#[test]
fn invalid_url() {
    let core = test_core_with_account();
    let mut account = core.get_account().unwrap();
    account.api_url = String::from("not a url");

    core.in_tx(|s| {
        let res = s
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() });
        assert_matches!(res, Err(ApiError::<GetPublicKeyError>::SendFailed(_)));
        Ok(())
    })
    .unwrap();
}

#[test]
fn wrong_url() {
    let core = test_core_with_account();
    let mut account = core.get_account().unwrap();
    account.api_url = String::from("http://google.com");

    core.in_tx(|s| {
        let result = s
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() });
        assert_matches!(result, Err(ApiError::<GetPublicKeyError>::Deserialize(_)));
        Ok(())
    })
    .unwrap();
}

// todo: test for invalid signature, signature mismatch during create account request
