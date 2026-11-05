//! Run with a local server in Stripe test mode: `cargo test -p lb-rs --test api_billing_tests -- --ignored`
use lb_rs::io::network::ApiError;
use lb_rs::model::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, StripeBillingPortalError,
    StripeBillingPortalRequest, StripeCheckoutRequest, StripePlan, UpgradeAccountGooglePlayError,
    UpgradeAccountGooglePlayRequest,
};
use test_utils::{assert_matches, test_core_with_account};

#[tokio::test]
#[ignore]
async fn upgrade_account_google_play_invalid_purchase_token() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

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
async fn stripe_checkout_is_a_stripe_page() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    for plan in [StripePlan::Monthly, StripePlan::Yearly] {
        let response = core
            .client
            .request(account, StripeCheckoutRequest { plan })
            .await
            .unwrap();
        assert!(response.url.starts_with("https://checkout.stripe.com/"), "{}", response.url);
    }
}

#[tokio::test]
#[ignore]
async fn stripe_billing_portal_needs_a_customer() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let result = core
        .client
        .request(account, StripeBillingPortalRequest {})
        .await;
    assert_matches!(
        result,
        Err(ApiError::<StripeBillingPortalError>::Endpoint(
            StripeBillingPortalError::NotStripeCustomer
        ))
    );

    // a checkout makes the account a stripe customer, paid or not
    core.client
        .request(account, StripeCheckoutRequest { plan: StripePlan::Monthly })
        .await
        .unwrap();
    let response = core
        .client
        .request(account, StripeBillingPortalRequest {})
        .await
        .unwrap();
    assert!(response.url.starts_with("https://billing.stripe.com/"), "{}", response.url);
}

#[tokio::test]
#[ignore]
async fn cancel_subscription_not_premium() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let result = core
        .client
        .request(account, CancelSubscriptionRequest {})
        .await;

    assert_matches!(
        result,
        Err(ApiError::<CancelSubscriptionError>::Endpoint(CancelSubscriptionError::NotPremium))
    );
}
