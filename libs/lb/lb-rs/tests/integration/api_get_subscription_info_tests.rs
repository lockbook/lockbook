use lb_rs::io::server::Server;
use lb_rs::model::api::{GetSubscriptionInfoRequest, GetSubscriptionInfoRequestV2};
use test_utils::test_core_with_account;

#[tokio::test]
async fn get_subscription_info_free() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();

    let v1 =
        Server::get_subscription_info(&core.client, account, GetSubscriptionInfoRequest {}).await;
    assert!(v1.unwrap().subscription_info.is_none());

    let v2 =
        Server::get_subscription_info_v2(&core.client, account, GetSubscriptionInfoRequestV2 {})
            .await;
    assert!(v2.unwrap().subscription_info.is_none());
}
