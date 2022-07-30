use lockbook_core::service::api_service;
use lockbook_core::service::api_service::ApiError;
use lockbook_shared::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, UpgradeAccountStripeRequest,
};
use lockbook_shared::file_metadata::FileType;
use rand::RngCore;
use test_utils::{
    assert_matches, generate_premium_account_tier, test_core_with_account, test_credit_cards,
};

#[test]
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
fn downgrade_denied() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();
    let root = core.yeah get_root().unwrap();

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
