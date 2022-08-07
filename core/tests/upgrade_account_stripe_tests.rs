use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::*;
use test_utils::*;

#[test]
fn upgrade_account_to_premium() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // upgrade account tier to premium
    api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();
}

#[test]
fn new_tier_is_old_tier() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // upgrade account tier to premium
    api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // upgrade account tier to premium
    let result = api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    );

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountStripeError>::Endpoint(
            UpgradeAccountStripeError::AlreadyPremium
        ))
    );
}

#[test]
fn card_does_not_exist() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // upgrade account tier to premium using an "old card"
    let result = api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: StripeAccountTier::Premium(PaymentMethod::OldCard),
        },
    );

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountStripeError>::Endpoint(
            UpgradeAccountStripeError::OldCardDoesNotExist
        ))
    );
}

#[test]
fn card_decline() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let scenarios = vec![
        (test_credit_cards::decline::GENERIC, UpgradeAccountStripeError::CardDecline),
        (test_credit_cards::decline::LOST_CARD, UpgradeAccountStripeError::CardDecline), // core should not be informed a card is stolen or lost
        (
            test_credit_cards::decline::INSUFFICIENT_FUNDS,
            UpgradeAccountStripeError::InsufficientFunds,
        ),
        (test_credit_cards::decline::PROCESSING_ERROR, UpgradeAccountStripeError::TryAgain),
        (test_credit_cards::decline::EXPIRED_CARD, UpgradeAccountStripeError::ExpiredCard),
    ];

    for (card_number, expected_err) in scenarios {
        // upgrade account tier to premium using bad card number
        let result = api_service::request(
            &account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(card_number, None, None, None),
            },
        );

        match result {
            Err(ApiError::<UpgradeAccountStripeError>::Endpoint(err)) => {
                assert_eq!(err, expected_err)
            }
            other => panic!("expected {:?}, got {:?}", expected_err, other),
        }
    }
}

#[test]
fn invalid_cards() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let scenarios = vec![
        (
            test_credit_cards::INVALID_NUMBER,
            None,
            None,
            None,
            UpgradeAccountStripeError::InvalidCardNumber,
        ),
        (
            test_credit_cards::GOOD,
            Some(1970),
            None,
            None,
            UpgradeAccountStripeError::InvalidCardExpYear,
        ),
        (
            test_credit_cards::GOOD,
            None,
            Some(14),
            None,
            UpgradeAccountStripeError::InvalidCardExpMonth,
        ),
        (
            test_credit_cards::GOOD,
            None,
            None,
            Some("11"),
            UpgradeAccountStripeError::InvalidCardCvc,
        ),
    ];

    for (card_number, maybe_exp_year, maybe_exp_month, maybe_cvc, expected_err) in scenarios {
        // upgrade account tier to premium using bad card information
        let result = api_service::request(
            &account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    card_number,
                    maybe_exp_year,
                    maybe_exp_month,
                    maybe_cvc,
                ),
            },
        );

        match result {
            Err(ApiError::<UpgradeAccountStripeError>::Endpoint(err)) => {
                assert_eq!(err, expected_err)
            }
            other => panic!("expected {:?}, got {:?}", expected_err, other),
        }
    }
}
