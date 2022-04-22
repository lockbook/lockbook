use google_androidpublisher3::{AndroidPublisher, Error};
use google_androidpublisher3::api::{ProductPurchasesAcknowledgeRequest, SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest};
use google_androidpublisher3::hyper::StatusCode;
use crate::ServerState;

pub enum SimpleGCPError {
    Unexpected(String)
}

const PACKAGE_NAME: &str = "app.lockbook";

async fn acknowledge_subscription(
    client: &AndroidPublisher,
    product_id: &str,
    purchase_token: &str,
    username: String,
) -> Result<(), SimpleGCPError> {
    let req = SubscriptionPurchasesAcknowledgeRequest { developer_payload: Some(username) };

    return match client
        .purchases()
        .subscriptions_acknowledge(req, PACKAGE_NAME, product_id, purchase_token)
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err)))
    };
}

async fn cancel_subscription(
    client: &AndroidPublisher,
    product_id: &str,
    purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    return match client
        .purchases()
        .subscriptions_cancel(PACKAGE_NAME, product_id, purchase_token)
        .doit()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err)))
    };
}

async fn verify_subscription(
    client: &AndroidPublisher,
    product_id: &str,
    purchase_token: &str,
) -> Result<SubscriptionPurchase, SimpleGCPError> {
    return match client
        .purchases()
        .subscriptions_get(PACKAGE_NAME, product_id, purchase_token)
        .doit()
        .await
    {
        Ok(resp) => Ok(resp.1),
        Err(err) => Err(SimpleGCPError::Unexpected(format!("{:#?}", err)))
    };
}
