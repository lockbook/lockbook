mod test_utils;
use crate::test_utils::{generate_premium_account_tier, test_core_with_account, test_credit_cards};
use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileType;
use rand::RngCore;

#[test]
fn switch_to_premium_and_back() {
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

    // switch account tier back to free
    api_service::request(&account, SwitchAccountTierRequest { account_tier: AccountTier::Free })
        .unwrap();
}

#[test]
fn new_tier_is_old_tier() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // switch account tier to free
    let result = api_service::request(
        &account,
        SwitchAccountTierRequest { account_tier: AccountTier::Free },
    );

    assert_matches!(
        result,
        Err(ApiError::<SwitchAccountTierError>::Endpoint(SwitchAccountTierError::NewTierIsOldTier))
    );

    // switch account tier to premium
    api_service::request(
        &account,
        SwitchAccountTierRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // switch account tier to premium
    let result = api_service::request(
        &account,
        SwitchAccountTierRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    );

    assert_matches!(
        result,
        Err(ApiError::<SwitchAccountTierError>::Endpoint(SwitchAccountTierError::NewTierIsOldTier))
    );
}

#[test]
fn card_does_not_exist() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // switch account tier to premium using an "old card"
    let result = api_service::request(
        &account,
        SwitchAccountTierRequest { account_tier: AccountTier::Premium(PaymentMethod::OldCard) },
    );

    assert_matches!(
        result,
        Err(ApiError::<SwitchAccountTierError>::Endpoint(
            SwitchAccountTierError::OldCardDoesNotExist
        ))
    );
}

#[test]
fn card_decline() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    let scenarios = vec![
        (test_credit_cards::decline::GENERIC, SwitchAccountTierError::CardDecline),
        (test_credit_cards::decline::LOST_CARD, SwitchAccountTierError::CardDecline), // core should not be informed a card is stolen or lost
        (test_credit_cards::decline::INSUFFICIENT_FUNDS, SwitchAccountTierError::InsufficientFunds),
        (test_credit_cards::decline::PROCESSING_ERROR, SwitchAccountTierError::TryAgain),
        (test_credit_cards::decline::EXPIRED_CARD, SwitchAccountTierError::ExpiredCard),
    ];

    for (card_number, expected_err) in scenarios {
        // switch account tier to premium using bad card number
        let result = api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_premium_account_tier(card_number, None, None, None),
            },
        );

        match result {
            Err(ApiError::<SwitchAccountTierError>::Endpoint(err)) => {
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
            SwitchAccountTierError::InvalidCardNumber,
        ),
        (
            test_credit_cards::GOOD,
            Some(1970),
            None,
            None,
            SwitchAccountTierError::InvalidCardExpYear,
        ),
        (
            test_credit_cards::GOOD,
            None,
            Some(14),
            None,
            SwitchAccountTierError::InvalidCardExpMonth,
        ),
        (test_credit_cards::GOOD, None, None, Some("11"), SwitchAccountTierError::InvalidCardCvc),
    ];

    for (card_number, maybe_exp_year, maybe_exp_month, maybe_cvc, expected_err) in scenarios {
        // switch account tier to premium using bad card information
        let result = api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_premium_account_tier(
                    card_number,
                    maybe_exp_year,
                    maybe_exp_month,
                    maybe_cvc,
                ),
            },
        );

        match result {
            Err(ApiError::<SwitchAccountTierError>::Endpoint(err)) => {
                assert_eq!(err, expected_err)
            }
            other => panic!("expected {:?}, got {:?}", expected_err, other),
        }
    }
}

#[test]
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
        SwitchAccountTierRequest {
            account_tier: generate_premium_account_tier(test_credit_cards::GOOD, None, None, None),
        },
    )
    .unwrap();

    // attempt to switch account tier back free
    let result = api_service::request(
        &account,
        SwitchAccountTierRequest { account_tier: AccountTier::Free },
    );

    assert_matches!(
        result,
        Err(ApiError::<SwitchAccountTierError>::Endpoint(
            SwitchAccountTierError::CurrentUsageIsMoreThanNewTier
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

    // switch account tier back to free
    api_service::request(&account, SwitchAccountTierRequest { account_tier: AccountTier::Free })
        .unwrap();
}
