use crate::config::Config;
use async_trait::async_trait;
use google_androidpublisher3::api::{
    SubscriptionPurchase, SubscriptionPurchasesAcknowledgeRequest,
};
use google_androidpublisher3::hyper::StatusCode;
use google_androidpublisher3::hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use google_androidpublisher3::hyper_util::client::legacy::Client;
use google_androidpublisher3::hyper_util::client::legacy::connect::HttpConnector;
use google_androidpublisher3::hyper_util::rt::TokioExecutor;
use google_androidpublisher3::yup_oauth2::client::CustomHyperClientBuilder;
use google_androidpublisher3::{AndroidPublisher, Error, yup_oauth2};

const PACKAGE_NAME: &str = "app.lockbook";

type Connector = HttpsConnector<HttpConnector>;

fn connector() -> Connector {
    HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build()
}

fn oauth_client() -> CustomHyperClientBuilder<Connector> {
    Client::builder(TokioExecutor::new())
        .build(connector())
        .into()
}

pub async fn get_google_play_client(
    service_account_key: &Option<String>,
) -> AndroidPublisher<Connector> {
    let auth = match service_account_key {
        Some(key) => {
            let service_account_key = yup_oauth2::parse_service_account_key(key).unwrap();

            yup_oauth2::ServiceAccountAuthenticator::with_client(
                service_account_key,
                oauth_client(),
            )
            .build()
            .await
            .unwrap()
        }
        None => {
            // creating dummy AndroidPublisher since no service account was provided
            yup_oauth2::InstalledFlowAuthenticator::with_client(
                yup_oauth2::ApplicationSecret::default(),
                yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
                oauth_client(),
            )
            .build()
            .await
            .unwrap()
        }
    };

    AndroidPublisher::new(Client::builder(TokioExecutor::new()).build(connector()), auth)
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
                SubscriptionPurchasesAcknowledgeRequest {
                    developer_payload: None,
                    external_account_ids: None,
                },
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
