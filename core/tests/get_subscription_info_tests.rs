use lockbook_core::service::api_service::Requester;
use lockbook_shared::api::{GetSubscriptionInfoRequest, UpgradeAccountStripeRequest};
use test_utils::{generate_premium_account_tier, test_core_with_account, test_credit_cards};

#[test]
fn get_subscription_info() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    // get no subscription info
    assert!(core
        .client
        .request(&account, GetSubscriptionInfoRequest {})
        .unwrap()
        .subscription_info
        .is_none());

    // upgrade with stripe
    core.client
        .request(
            &account,
            UpgradeAccountStripeRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .unwrap();

    // get existent subscription info
    assert!(core
        .client
        .request(&account, GetSubscriptionInfoRequest {})
        .unwrap()
        .subscription_info
        .is_some());
}
