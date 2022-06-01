use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::AndroidPublisher;

pub enum SimpleGCPError {
    Unexpected(String),
}

const PACKAGE_NAME: &str = "app.lockbook";

pub async fn acknowledge_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    match client
        .purchases()
        .subscriptions_acknowledge(
            SubscriptionPurchasesAcknowledgeRequest { developer_payload: None },
            PACKAGE_NAME,
            subscription_id,
            purchase_token,
        )
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    }
}

pub async fn cancel_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    match client
        .purchases()
        .subscriptions_cancel(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    }
}

pub async fn get_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<SubscriptionPurchase, SimpleGCPError> {
    match client
        .purchases()
        .subscriptions_get(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await
    {
        Ok(resp) => Ok(resp.1),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    }
}
