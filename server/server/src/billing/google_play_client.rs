use crate::config::Config;
use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::hyper::StatusCode;
use google_androidpublisher3::{hyper, hyper_rustls, oauth2, AndroidPublisher, Error};

const PACKAGE_NAME: &str = "app.lockbook";

pub async fn get_google_play_client(service_account_key: &Option<String>) -> AndroidPublisher {
    let auth = match service_account_key {
        Some(key) => {
            let service_account_key: oauth2::ServiceAccountKey =
                oauth2::parse_service_account_key(key).unwrap();

            oauth2::ServiceAccountAuthenticator::builder(service_account_key)
                .build()
                .await
                .unwrap()
        }
        None => {
            // creating dummy AndroidPublisher since no service account was provided
            oauth2::InstalledFlowAuthenticator::builder(
                oauth2::ApplicationSecret::default(),
                oauth2::InstalledFlowReturnMethod::HTTPRedirect,
            )
            .build()
            .await
            .unwrap()
        }
    };

    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::with_native_roots(Default::default())
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build(),
    );

    AndroidPublisher::new(client, auth)
}

#[derive(Debug)]
pub enum SimpleGCPError {
    PurchaseTokenNotFound,
    Unexpected(String),
}

impl From<Error> for SimpleGCPError {
    fn from(err: Error) -> Self {
        match err {
            Error::Failure(err) => {
                if err.status() == StatusCode::NOT_FOUND {
                    Self::PurchaseTokenNotFound
                } else {
                    Self::Unexpected(format!("{:#?}", err))
                }
            }
            _ => Self::Unexpected(format!("{:#?}", err)),
        }
    }
}

pub async fn acknowledge_subscription(
    config: &Config, client: &AndroidPublisher, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    let subscription_id = &config.billing.google.premium_subscription_product_id;
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
    config: &Config, client: &AndroidPublisher, purchase_token: &str,
) -> Result<(), SimpleGCPError> {
    let subscription_id = &config.billing.google.premium_subscription_product_id;
    client
        .purchases()
        .subscriptions_cancel(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await?;

    Ok(())
}

pub async fn get_subscription(
    config: &Config, client: &AndroidPublisher, purchase_token: &str,
) -> Result<SubscriptionPurchase, SimpleGCPError> {
    let subscription_id = &config.billing.google.premium_subscription_product_id;
    Ok(client
        .purchases()
        .subscriptions_get(PACKAGE_NAME, subscription_id, purchase_token)
        .doit()
        .await?
        .1)
}
