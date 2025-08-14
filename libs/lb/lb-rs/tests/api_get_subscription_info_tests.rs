use lb_rs::model::api::{GetSubscriptionInfoRequest, UpgradeAccountStripeRequest};
use test_utils::{generate_premium_account_tier, test_core_with_account, test_credit_cards};

#[tokio::test]
#[ignore]
async fn get_subscription_info() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    assert!(
        core.client
            .request(account, GetSubscriptionInfoRequest {})
            .await
            .unwrap()
            .subscription_info
            .is_none()
    );

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

    assert!(
        core.client
            .request(account, GetSubscriptionInfoRequest {})
            .await
            .unwrap()
            .subscription_info
            .is_some()
    );
}
