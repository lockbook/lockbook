use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, PaymentMethod, StripeAccountTier,
    UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest, UpgradeAccountStripeError,
    UpgradeAccountStripeRequest,
};
use lockbook_shared::file_metadata::FileType;
use rand::RngCore;
use test_utils::{
    assert_matches, generate_premium_account_tier, test_core_with_account, test_credit_cards,
};

#[test]
#[ignore]
/// Run all tests with: cargo test --package lockbook-core --test billing_tests "" -- --ignored
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

#[test]
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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

#[test]
#[ignore]
fn cancel_stripe_subscription() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // switch account tier to premium
    api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // cancel stripe subscription
    api_service::request(&account, CancelSubscriptionRequest {}).unwrap();
}

#[test]
#[ignore]
fn downgrade_denied() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.get_root().unwrap();

    // create files until the account is over the 1mb data cap
    loop {
        let mut bytes: [u8; 500000] = [0u8; 500000];

        rand::thread_rng().fill_bytes(&mut bytes);

        let file = core
            .create_file(&uuid::Uuid::new_v4().to_string(), root.id, FileType::Document)
            .unwrap();
        core.write_document(file.id, &bytes).unwrap();
        core.sync(None).unwrap();

        // TODO: Currently a users data cap isn't enforced by the server. When it is, this code needs to be updated to not violate usage before upgrading.
        if core.get_usage().unwrap().server_usage.exact > 1000000 {
            break;
        }
    }

    // switch account tier to premium
    api_service::request(
        &account,
        UpgradeAccountStripeRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // attempt to cancel subscription but fail
    let result = api_service::request(&account, CancelSubscriptionRequest {});

    assert_matches!(
        result,
        Err(ApiError::<CancelSubscriptionError>::Endpoint(
            CancelSubscriptionError::UsageIsOverFreeTierDataCap
        ))
    );

    let children = core.get_children(root.id).unwrap();

    // delete files until the account is under the 1mb data cap
    for child in children {
        core.delete_file(child.id).unwrap();

        core.sync(None).unwrap();

        if core.get_usage().unwrap().server_usage.exact < 1000000 {
            break;
        }
    }

    // cancel subscription again
    api_service::request(&account, CancelSubscriptionRequest {}).unwrap();
}

#[test]
#[ignore]
fn cancel_subscription_not_premium() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // cancel subscription but the account is not premium
    let result = api_service::request(&account, CancelSubscriptionRequest {});

    assert_matches!(
        result,
        Err(ApiError::<CancelSubscriptionError>::Endpoint(CancelSubscriptionError::NotPremium))
    );
}
