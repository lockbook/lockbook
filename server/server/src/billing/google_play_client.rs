use crate::keys;
use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::AndroidPublisher;
use libsecp256k1::PublicKey;

pub enum SimpleGCPError {
    Unexpected(String),
}

const PACKAGE_NAME: &str = "app.lockbook";

pub async fn acknowledge_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str, public_key: &PublicKey,
) -> Result<(), SimpleGCPError> {
    let req = SubscriptionPurchasesAcknowledgeRequest {
        developer_payload: Some(
            serde_json::to_string(&public_key)
                .map_err(|e| SimpleGCPError::Unexpected(format!("{:#?}", e)))?,
        ),
    };

    return match client
        .purchases()
        .subscriptions_acknowledge(req, PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    };
}

pub async fn cancel_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    return match client
        .purchases()
        .subscriptions_cancel(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    };
}

pub async fn get_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<SubscriptionPurchase, SimpleGCPError> {
    return match client
        .purchases()
        .subscriptions_get(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await
    {
        Ok(resp) => Ok(resp.1),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err))),
    };
}
