use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_models::api::*;
use test_utils::*;

#[test]
fn get_credit_card() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // switch account tier to premium
    api_service::request(
        &account,
        SwitchAccountTierRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // get the last 4 digits of the most recently added card
    let result = api_service::request(&account, GetCreditCardRequest {})
        .unwrap()
        .credit_card_last_4_digits;

    assert_matches!(result.as_str(), test_credit_cards::GOOD_LAST_4);
}

#[test]
fn not_a_stripe_customer() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // attempt to get the last 4 digits of the most recently added card
    let result = api_service::request(&account, GetCreditCardRequest {});

    assert_matches!(
        result,
        Err(ApiError::<GetCreditCardError>::Endpoint(GetCreditCardError::NotAStripeCustomer))
    );
}
