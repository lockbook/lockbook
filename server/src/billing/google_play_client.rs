use crate::config::Config;
use async_trait::async_trait;
use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::hyper::StatusCode;
use google_androidpublisher3::hyper::client::HttpConnector;
use google_androidpublisher3::hyper_rustls::HttpsConnector;
use google_androidpublisher3::{AndroidPublisher, Error, hyper, hyper_rustls, oauth2};

const PACKAGE_NAME: &str = "app.lockbook";

pub async fn get_google_play_client(
    service_account_key: &Option<String>,
) -> AndroidPublisher<HttpsConnector<HttpConnector>> {
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
                    Self::Unexpected(format!("{err:#?}"))
                }
            }
            _ => Self::Unexpected(format!("{err:#?}")),
        }
    }
}

#[async_trait]
pub trait GooglePlayClient: Send + Sync + Clone + 'static {
    async fn acknowledge_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<(), SimpleGCPError>;

    async fn cancel_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<(), SimpleGCPError>;

    async fn get_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<SubscriptionPurchase, SimpleGCPError>;
}

#[async_trait]
impl GooglePlayClient for AndroidPublisher<HttpsConnector<HttpConnector>> {
    async fn acknowledge_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<(), SimpleGCPError> {
        let subscription_id = &config.billing.google.premium_subscription_product_id;
        self.purchases()
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

    async fn cancel_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<(), SimpleGCPError> {
        let subscription_id = &config.billing.google.premium_subscription_product_id;
        self.purchases()
            .subscriptions_cancel(PACKAGE_NAME, subscription_id, purchase_token)
            .doit()
            .await?;

        Ok(())
    }

    async fn get_subscription(
        &self, config: &Config, purchase_token: &str,
    ) -> Result<SubscriptionPurchase, SimpleGCPError> {
        let subscription_id = &config.billing.google.premium_subscription_product_id;
        Ok(self
            .purchases()
            .subscriptions_get(PACKAGE_NAME, subscription_id, purchase_token)
            .doit()
            .await?
            .1)
    }
}
