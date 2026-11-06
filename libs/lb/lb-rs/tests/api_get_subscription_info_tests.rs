use lb_rs::model::api::{GetSubscriptionInfoRequest, GetSubscriptionInfoRequestV2};
use test_utils::test_core_with_account;

#[tokio::test]
#[ignore]
async fn get_subscription_info_free() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let v1 = core
        .client
        .request(account, GetSubscriptionInfoRequest {})
        .await;
    assert!(v1.unwrap().subscription_info.is_none());

    let v2 = core
        .client
        .request(account, GetSubscriptionInfoRequestV2 {})
        .await;
    assert!(v2.unwrap().subscription_info.is_none());
}
