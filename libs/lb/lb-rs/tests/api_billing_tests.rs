use lb_rs::io::network::ApiError;
use lb_rs::model::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, FREE_TIER_USAGE_SIZE, PaymentMethod,
    StripeAccountTier, UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};
use lb_rs::model::file_metadata::FileType;
use rand::RngCore;
use test_utils::{
    assert_matches, generate_premium_account_tier, test_core_with_account, test_credit_cards,
};

#[tokio::test]
#[ignore]
/// Run all tests with: cargo test --package lockbook-core --test billing_tests "" -- --ignored
async fn upgrade_account_google_play_already_premium() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // upgrade account tier to premium using stripe
    core.client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await
        .unwrap();

    // try to upgrade to premium with android
    let result = core
        .client
        .request(
            account,
            UpgradeAccountGooglePlayRequest {
                purchase_token: "".to_string(),
                account_id: "".to_string(),
            },
        )
        .await;

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountGooglePlayError>::Endpoint(
            UpgradeAccountGooglePlayError::AlreadyPremium
        ))
    );
}

#[tokio::test]
#[ignore]
async fn upgrade_account_google_play_invalid_purchase_token() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // upgrade with bad purchase token
    let result = core
        .client
        .request(
            account,
            UpgradeAccountGooglePlayRequest {
                purchase_token: "".to_string(),
                account_id: "".to_string(),
            },
        )
        .await;

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountGooglePlayError>::Endpoint(
            UpgradeAccountGooglePlayError::InvalidPurchaseToken
        ))
    );
}

#[tokio::test]
#[ignore]
async fn upgrade_account_to_premium() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // upgrade account tier to premium
    core.client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn new_tier_is_old_tier() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // upgrade account tier to premium
    core.client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await
        .unwrap();

    // upgrade account tier to premium
    let result = core
        .client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await;

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountStripeError>::Endpoint(
            UpgradeAccountStripeError::AlreadyPremium
        ))
    );
}

#[tokio::test]
#[ignore]
async fn card_does_not_exist() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // upgrade account tier to premium using an "old card"
    let result = core
        .client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: StripeAccountTier::Premium(PaymentMethod::OldCard),
            },
        )
        .await;

    assert_matches!(
        result,
        Err(ApiError::<UpgradeAccountStripeError>::Endpoint(
            UpgradeAccountStripeError::OldCardDoesNotExist
        ))
    );
}

#[tokio::test]
#[ignore]
async fn card_decline() {
    let core = test_core_with_account().await;
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
        let result = core
            .client
            .request(
                account,
                UpgradeAccountStripeRequest {
                    account_tier: generate_premium_account_tier(card_number, None, None, None),
                },
            )
            .await;

        match result {
            Err(ApiError::<UpgradeAccountStripeError>::Endpoint(err)) => {
                assert_eq!(err, expected_err)
            }
            other => panic!("expected {expected_err:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
#[ignore]
async fn invalid_cards() {
    let core = test_core_with_account().await;
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
        let result = core
            .client
            .request(
                account,
                UpgradeAccountStripeRequest {
                    account_tier: generate_premium_account_tier(
                        card_number,
                        maybe_exp_year,
                        maybe_exp_month,
                        maybe_cvc,
                    ),
                },
            )
            .await;

        match result {
            Err(ApiError::<UpgradeAccountStripeError>::Endpoint(err)) => {
                assert_eq!(err, expected_err)
            }
            other => panic!("expected {expected_err:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
#[ignore]
async fn cancel_stripe_subscription() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // switch account tier to premium
    core.client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await
        .unwrap();

    // cancel stripe subscription
    core.client
        .request(account, CancelSubscriptionRequest {})
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn downgrade_denied() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let root = core.root().await.unwrap();

    // create files until the account is over the 1mb data cap
    loop {
        let mut bytes: [u8; 500000] = [0u8; 500000];

        rand::thread_rng().fill_bytes(&mut bytes);

        if core.get_usage().await.unwrap().server_usage.exact
            > (FREE_TIER_USAGE_SIZE as f64 * 0.5) as u64
        {
            break;
        }

        let file = core
            .create_file(&uuid::Uuid::new_v4().to_string(), &root.id, FileType::Document)
            .await
            .unwrap();
        core.write_document(file.id, &bytes).await.unwrap();

        core.sync(None).await.unwrap();
    }

    // switch account tier to premium
    core.client
        .request(
            account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .await
        .unwrap();

    // go over free tier
    let file = core
        .create_file(&uuid::Uuid::new_v4().to_string(), &root.id, FileType::Document)
        .await
        .unwrap();

    let content: Vec<u8> = (0..(FREE_TIER_USAGE_SIZE))
        .map(|_| rand::random::<u8>())
        .collect();
    core.write_document(file.id, &content).await.unwrap();
    core.sync(None).await.unwrap();

    // attempt to cancel subscription but fail
    let result = core
        .client
        .request(account, CancelSubscriptionRequest {})
        .await;

    assert_matches!(
        result,
        Err(ApiError::<CancelSubscriptionError>::Endpoint(
            CancelSubscriptionError::UsageIsOverFreeTierDataCap
        ))
    );

    let children = core.get_children(&root.id).await.unwrap();

    // delete files until the account is under the 1mb data cap
    for child in children {
        core.delete(&child.id).await.unwrap();

        core.sync(None).await.unwrap();

        if core.get_usage().await.unwrap().server_usage.exact < FREE_TIER_USAGE_SIZE {
            break;
        }
    }

    // cancel subscription again
    core.client
        .request(account, CancelSubscriptionRequest {})
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn cancel_subscription_not_premium() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    // cancel subscription but the account is not premium
    let result = core
        .client
        .request(account, CancelSubscriptionRequest {})
        .await;

    assert_matches!(
        result,
        Err(ApiError::<CancelSubscriptionError>::Endpoint(CancelSubscriptionError::NotPremium))
    );
}
