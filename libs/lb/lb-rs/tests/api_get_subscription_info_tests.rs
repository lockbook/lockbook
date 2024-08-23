use lb_rs::logic::api::{GetSubscriptionInfoRequest, UpgradeAccountStripeRequest};
use lb_rs::service::api_service::Requester;
use test_utils::{generate_premium_account_tier, test_core_with_account, test_credit_cards};

#[test]
fn get_subscription_info() {
    let core = test_core_with_account();
    let account = core.get_account().unwrap();

    core.in_tx(|s| {
        assert!(s
            .client
            .request(&account, GetSubscriptionInfoRequest {})
            .unwrap()
            .subscription_info
            .is_none());

        s.client
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

        assert!(s
            .client
            .request(&account, GetSubscriptionInfoRequest {})
            .unwrap()
            .subscription_info
            .is_some());
        Ok(())
    })
    .unwrap();
}
