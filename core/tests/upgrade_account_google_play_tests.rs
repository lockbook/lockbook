use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::{
    UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest, UpgradeAccountStripeRequest,
};
use test_utils::{
    assert_matches, generate_premium_account_tier, test_core_with_account, test_credit_cards,
};

#[test]
fn upgrade_account_google_play_already_premium() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // upgrade account tier to premium using stripe
    api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // try to upgrade to premium with android
    let result = api_service::request(
        &account,
        UpgradeAccountGooglePlayRequest {
            purchase_token: "".to_string(),
            account_id: "".to_string(),
        },
    );

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountGooglePlayError>::Endpoint(
            UpgradeAccountGooglePlayError::AlreadyPremium
        ))
    );
}

#[test]
#[ignore]
fn upgrade_account_google_play_invalid_purchase_token() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // upgrade with bad purchase token
    let result = api_service::request(
        &account,
        UpgradeAccountGooglePlayRequest {
            purchase_token: "".to_string(),
            account_id: "".to_string(),
        },
    );

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountGooglePlayError>::Endpoint(
            UpgradeAccountGooglePlayError::InvalidPurchaseToken
        ))
    );
}
