use crate::config::Environment::{Local, Prod, Unknown};
use std::fmt::Display;
use std::time::Duration;
use std::{env, fmt};

#[derive(Clone)]
pub struct IndexDbConf {
    pub redis_url: String,
}

impl IndexDbConf {
    pub fn from_env_vars() -> Self {
        Self { redis_url: env_or_panic("INDEX_DB_REDIS_URL") }
    }
}

#[derive(Clone)]
pub struct FilesDbConfig {
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl FilesDbConfig {
    pub fn from_env_vars() -> FilesDbConfig {
        FilesDbConfig {
            scheme: env_or_empty("FILES_DB_SCHEME"),
            host: env_or_empty("FILES_DB_HOST"),
            port: env_or_empty("FILES_DB_PORT").map(|e| e.parse().expect("Expected u16!")),
            region: env_or_panic("FILES_DB_REGION").parse().unwrap(),
            bucket: env_or_panic("FILES_DB_BUCKET"),
            access_key: env_or_panic("FILES_DB_ACCESS_KEY"),
            secret_key: env_or_panic("FILES_DB_SECRET_KEY"),
        }
    }
}

#[derive(Clone)]
pub struct StripeConfig {
    pub stripe_secret: String,
    pub signing_secret: String,
    pub premium_price_id: String,
    pub millis_between_user_payment_flows: u64,
}

impl StripeConfig {
    pub fn from_env_vars() -> StripeConfig {
        StripeConfig {
            stripe_secret: env_or_panic("STRIPE_SECRET").parse().unwrap(),
            signing_secret: env_or_panic("STRIPE_SIGNING_SECRET").parse().unwrap(),
            premium_price_id: env_or_panic("STRIPE_PREMIUM_PRICE_ID").parse().unwrap(),
            millis_between_user_payment_flows: env_or_panic("MILLIS_BETWEEN_PAYMENT_FLOWS")
                .parse()
                .unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Environment {
    Prod,
    Local,
    Unknown,
}

impl Environment {
    pub fn from_env_vars() -> Self {
        match env::var("ENVIRONMENT") {
            Ok(var) => match var.to_lowercase().as_str() {
                "production" | "prod" => Prod,
                "local" | "localhost" => Local,
                _ => Unknown,
            },
            Err(_) => Unknown,
        }
    }
}

impl Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", &self)
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub env: Environment,
    pub port: u16,
    pub max_auth_delay: u128,
    pub log_path: String,
    pub pd_api_key: Option<String>,
    pub ssl_cert_location: Option<String>,
    pub ssl_private_key_location: Option<String>,
}

impl ServerConfig {
    pub fn from_env_vars() -> ServerConfig {
        let env = Environment::from_env_vars();
        let port = env_or_panic("SERVER_PORT").parse().unwrap();
        let max_auth_delay = env_or_panic("MAX_AUTH_DELAY").parse().unwrap();
        let log_path = env_or_panic("LOG_PATH").parse().unwrap();
        let pd_api_key = env_or_empty("PD_KEY");
        let ssl_cert_location = env_or_empty("SSL_CERT_LOCATION");
        let ssl_private_key_location = env_or_empty("SSL_PRIVATE_KEY_LOCATION");

        match (&pd_api_key, &ssl_cert_location, &ssl_private_key_location) {
            (Some(_), Some(_), Some(_)) | (None, None, None) => {}
            _ => panic!(
                "Invalid config, pd & ssl must all be Some (production) or all be None (local)"
            ),
        }

        ServerConfig {
            env,
            port,
            max_auth_delay,
            log_path,
            pd_api_key,
            ssl_cert_location,
            ssl_private_key_location,
        }
    }
}

#[derive(Clone)]
pub struct MetricsConfig {
    pub time_between_metrics_refresh: Duration,
    pub time_between_redis_calls: Duration,
}

impl MetricsConfig {
    pub fn from_env_vars() -> MetricsConfig {
        MetricsConfig {
            time_between_metrics_refresh: Duration::from_secs(
                env_or_panic("MINUTES_BETWEEN_METRICS_REFRESH")
                    .parse::<u64>()
                    .unwrap()
                    * 60,
            ),
            time_between_redis_calls: Duration::from_millis(
                env_or_panic("MILLIS_BETWEEN_REDIS_CALLS")
                    .parse::<u64>()
                    .unwrap(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct GoogleConfig {
    pub service_account_cred_path: Option<String>,
    pub premium_subscription_product_id: String,
    pub premium_subscription_offer_id: String,
    pub pubsub_token: String,
}

impl GoogleConfig {
    pub fn from_env_vars() -> GoogleConfig {
        GoogleConfig {
            service_account_cred_path: env_or_empty("GOOGLE_CLOUD_SERVICE_ACCOUNT_CRED_PATH"),
            premium_subscription_product_id: env_or_panic(
                "GOOGLE_PLAY_PREMIUM_SUBSCRIPTION_PRODUCT_ID",
            ),
            premium_subscription_offer_id: env_or_panic(
                "GOOGLE_PLAY_PREMIUM_SUBSCRIPTION_OFFER_ID",
            ),
            pubsub_token: env_or_panic("GOOGLE_CLOUD_PUBSUB_NOTIFICATION_TOKEN"),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub index_db: IndexDbConf,
    pub files_db: FilesDbConfig,
    pub stripe: StripeConfig,
    pub metrics: MetricsConfig,
    pub google: GoogleConfig,
}

impl Config {
    pub fn from_env_vars() -> Self {
        Self {
            index_db: IndexDbConf::from_env_vars(),
            files_db: FilesDbConfig::from_env_vars(),
            server: ServerConfig::from_env_vars(),
            stripe: StripeConfig::from_env_vars(),
            metrics: MetricsConfig::from_env_vars(),
            google: GoogleConfig::from_env_vars(),
        }
    }

    pub fn is_prod(&self) -> bool {
        self.server.pd_api_key.is_some()
            && self.server.ssl_private_key_location.is_some()
            && self.server.ssl_cert_location.is_some()
    }
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).unwrap_or_else(|_| panic!("Missing environment variable {}", var_name))
}

fn env_or_empty(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(var) => Some(var),
        Err(err) => {
            eprintln!("env var error. var={:?}, err={:?}", var_name, err);
            None
        }
    }
}
