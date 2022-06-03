use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::hyper::StatusCode;
use google_androidpublisher3::{AndroidPublisher, Error};

#[derive(Debug)]
pub enum SimpleGCPError {
    PurchaseTokenNotFound,
    Unexpected(String),
}

impl From<google_androidpublisher3::Error> for SimpleGCPError {
    fn from(err: Error) -> Self {
        match err {
            Error::Failure(err) => {
                if err.status() == StatusCode::NOT_FOUND {
                    SimpleGCPError::PurchaseTokenNotFound
                } else {
                    SimpleGCPError::Unexpected(format!("{:#?}", err))
                }
            }
            _ => SimpleGCPError::Unexpected(format!("{:#?}", err)),
        }
    }
}

const PACKAGE_NAME: &str = "app.lockbook";

pub async fn acknowledge_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    client
        .purchases()
        .subscriptions_acknowledge(
            SubscriptionPurchasesAcknowledgeRequest { developer_payload: None },
            PACKAGE_NAME,
            subscription_id,
            purchase_token,
        )
        .doit()
        .await?;

    Ok(())
}

pub async fn cancel_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    client
        .purchases()
        .subscriptions_cancel(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await?;

    Ok(())
}

pub async fn get_subscription(
    client: &AndroidPublisher, subscription_id: &str, purchase_token: &str,
) -> Result<SubscriptionPurchase, SimpleGCPError> {
    Ok(client
        .purchases()
        .subscriptions_get(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await?
        .1)
}
