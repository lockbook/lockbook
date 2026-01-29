use libsecp256k1::PublicKey;

use lb_rs::get_code_version;
use lb_rs::io::network::{ApiError, Network};
use lb_rs::model::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};
use lb_rs::model::clock::{Timestamp, get_time};
use test_utils::{assert_matches, test_core_with_account};

static CODE_VERSION: fn() -> &'static str = || "0.0.0";

#[tokio::test]
async fn forced_upgrade() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let client = Network { client: Default::default(), get_code_version: CODE_VERSION, get_time };

    let result: Result<PublicKey, ApiError<GetPublicKeyError>> = client
        .request(account, GetPublicKeyRequest { username: account.username.clone() })
        .await
        .map(|r: GetPublicKeyResponse| r.key);

    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ClientUpdateRequired));
}

static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(get_time().0 - 3600000);

#[tokio::test]
async fn expired_request() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let client = Network { client: Default::default(), get_code_version, get_time: EARLY_CLOCK };

    let result = client
        .request(account, GetPublicKeyRequest { username: account.username.clone() })
        .await;
    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::ExpiredAuth));
}

#[tokio::test]
async fn invalid_url() {
    let core = test_core_with_account().await;
    let mut account = core.get_account().unwrap().clone();
    account.api_url = String::from("not a url");

    let res = core
        .client
        .request(&account, GetPublicKeyRequest { username: account.username.clone() })
        .await;
    assert_matches!(res, Err(ApiError::<GetPublicKeyError>::SendFailed(_)));
}

#[tokio::test]
async fn wrong_url() {
    let core = test_core_with_account().await;
    let mut account = core.get_account().unwrap().clone();
    account.api_url = String::from("http://google.com");

    let result = core
        .client
        .request(&account, GetPublicKeyRequest { username: account.username.clone() })
        .await;
    assert_matches!(result, Err(ApiError::<GetPublicKeyError>::Deserialize(_)));
}

// todo: test for invalid signature, signature mismatch during create account request
