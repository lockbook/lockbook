use crate::config::Environment::{Local, Prod, Unknown};
use lb_rs::model::account::Username;
use semver::VersionReq;
use std::collections::HashSet;
use std::fmt::Display;
use std::path::PathBuf;
use std::time::Duration;
use std::{env, fmt, fs};

#[derive(Clone, Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub index_db: IndexDbConf,
    pub files: FilesConfig,
    pub metrics: MetricsConfig,
    pub billing: BillingConfig,
    pub admin: AdminConfig,
    pub features: FeatureFlags,
}

impl Config {
    pub fn from_env_vars() -> Self {
        Self {
            index_db: IndexDbConf::from_env_vars(),
            files: FilesConfig::from_env_vars(),
            server: ServerConfig::from_env_vars(),
            metrics: MetricsConfig::from_env_vars(),
            billing: BillingConfig::from_env_vars(),
            admin: AdminConfig::from_env_vars(),
            features: FeatureFlags::from_env_vars(),
        }
    }

    pub fn is_prod(&self) -> bool {
        self.server.env == Prod
    }
}

#[derive(Clone, Debug)]
pub struct IndexDbConf {
    pub db_location: String,
    pub time_between_compacts: Duration,
}

impl IndexDbConf {
    pub fn from_env_vars() -> Self {
        Self {
            db_location: env_or_panic("INDEX_DB_LOCATION"),
            time_between_compacts: Duration::from_secs(
                env_or_panic("MINUTES_BETWEEN_BACKGROUND_COMPACTS")
                    .parse::<u64>()
                    .unwrap()
                    * 60,
            ),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AdminConfig {
    pub admins: HashSet<Username>,
}

impl AdminConfig {
    pub fn from_env_vars() -> Self {
        Self {
            admins: env::var("ADMINS")
                .unwrap_or_else(|_| "".to_string())
                .split(", ")
                .map(|part| part.to_string())
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FeatureFlags {
    pub new_accounts: bool,
}

impl FeatureFlags {
    pub fn from_env_vars() -> Self {
        Self {
            new_accounts: env::var("FEATURE_NEW_ACCOUNTS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FilesConfig {
    pub path: PathBuf,
}

impl FilesConfig {
    pub fn from_env_vars() -> Self {
        let path = env_or_panic("FILES_PATH");
        let path = PathBuf::from(path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub env: Environment,
    pub port: u16,
    pub max_auth_delay: u128,
    pub log_path: String,
    pub pd_api_key: Option<String>,
    pub ssl_cert_location: Option<String>,
    pub ssl_private_key_location: Option<String>,
    pub min_core_version: VersionReq,
}

impl ServerConfig {
    pub fn from_env_vars() -> Self {
        let env = Environment::from_env_vars();
        let port = env_or_panic("SERVER_PORT").parse().unwrap();
        let max_auth_delay = env_or_panic("MAX_AUTH_DELAY").parse().unwrap();
        let log_path = env_or_panic("LOG_PATH").parse().unwrap();
        let pd_api_key = env_or_empty("PD_KEY");
        let ssl_cert_location = env_or_empty("SSL_CERT_LOCATION");
        let ssl_private_key_location = env_or_empty("SSL_PRIVATE_KEY_LOCATION");
        let min_core_version = VersionReq::parse(&env_or_panic("MIN_CORE_VERSION")).unwrap();

        match (&pd_api_key, &ssl_cert_location, &ssl_private_key_location) {
            (Some(_), Some(_), Some(_)) | (None, None, None) => {}
            _ => panic!(
                "Invalid config, pd & ssl must all be Some (production) or all be None (local)"
            ),
        }

        Self {
            env,
            port,
            max_auth_delay,
            log_path,
            pd_api_key,
            ssl_cert_location,
            ssl_private_key_location,
            min_core_version,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub time_between_metrics_refresh: Duration,
    pub time_between_metrics: Duration,
}

impl MetricsConfig {
    pub fn from_env_vars() -> Self {
        Self {
            time_between_metrics_refresh: Duration::from_secs(
                env_or_panic("MINUTES_BETWEEN_METRICS_REFRESH")
                    .parse::<u64>()
                    .unwrap()
                    * 60,
            ),
            time_between_metrics: Duration::from_millis(
                env_or_panic("MILLIS_BETWEEN_METRICS")
                    .parse::<u64>()
                    .unwrap(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BillingConfig {
    pub millis_between_user_payment_flows: u64,
    pub time_between_lock_attempts: Duration,
    pub google: GoogleConfig,
    pub stripe: StripeConfig,
    pub apple: AppleConfig,
}

impl BillingConfig {
    pub fn from_env_vars() -> Self {
        Self {
            millis_between_user_payment_flows: env_or_panic("MILLIS_BETWEEN_PAYMENT_FLOWS")
                .parse()
                .unwrap(),
            time_between_lock_attempts: Duration::from_secs(
                env_or_panic("MILLIS_BETWEEN_LOCK_ATTEMPTS")
                    .parse::<u64>()
                    .unwrap(),
            ),
            google: GoogleConfig::from_env_vars(),
            stripe: StripeConfig::from_env_vars(),
            apple: AppleConfig::from_env_vars(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppleConfig {
    pub iap_key: String,
    pub iap_key_id: String,
    pub asc_public_key: String,
    pub issuer_id: String,
    pub subscription_product_id: String,
    pub asc_shared_secret: String,
    pub apple_root_cert: Vec<u8>,
    pub monthly_sub_group_id: String,
}

impl AppleConfig {
    pub fn from_env_vars() -> Self {
        let is_apple_prod = env_or_empty("IS_APPLE_PROD")
            .map(|is_apple_prod| is_apple_prod.parse().unwrap())
            .unwrap_or(false);

        let apple_root_cert =
            env_or_empty("APPLE_ROOT_CERT_PATH").map(|cert_path| fs::read(cert_path).unwrap());
        let apple_iap_key = env_or_empty("APPLE_IAP_KEY_PATH")
            .map(|key_path| fs::read_to_string(key_path).unwrap());
        let apple_asc_pub_key = env_or_empty("APPLE_ASC_PUB_KEY_PATH")
            .map(|key_path| fs::read_to_string(key_path).unwrap());

        Self {
            iap_key: if is_apple_prod {
                apple_iap_key.unwrap()
            } else {
                apple_iap_key.unwrap_or_default()
            },
            iap_key_id: env_or_panic("APPLE_IAP_KEY_ID"),
            asc_public_key: if is_apple_prod {
                apple_asc_pub_key.unwrap()
            } else {
                apple_asc_pub_key.unwrap_or_default()
            },
            issuer_id: env_or_panic("APPLE_ISSUER_ID"),
            subscription_product_id: env_or_panic("APPLE_SUB_PROD_ID"),
            asc_shared_secret: env_or_panic("APPLE_ASC_SHARED_SECRET"),
            apple_root_cert: if is_apple_prod {
                apple_root_cert.unwrap()
            } else {
                apple_root_cert.unwrap_or_default()
            },
            monthly_sub_group_id: env_or_panic("APPLE_MONTHLY_SUB_GROUP_ID"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GoogleConfig {
    pub service_account_key: Option<String>,
    pub premium_subscription_product_id: String,
    pub premium_subscription_offer_id: String,
    pub pubsub_token: String,
}

impl GoogleConfig {
    pub fn from_env_vars() -> Self {
        Self {
            service_account_key: env_or_empty("GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY"),
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

#[derive(Clone, Debug)]
pub struct StripeConfig {
    pub stripe_secret: String,
    pub signing_secret: String,
    pub premium_price_id: String,
}

impl StripeConfig {
    pub fn from_env_vars() -> Self {
        Self {
            stripe_secret: env_or_panic("STRIPE_SECRET").parse().unwrap(),
            signing_secret: env_or_panic("STRIPE_SIGNING_SECRET").parse().unwrap(),
            premium_price_id: env_or_panic("STRIPE_PREMIUM_PRICE_ID").parse().unwrap(),
        }
    }
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).unwrap_or_else(|_| panic!("Missing environment variable {var_name}"))
}

fn env_or_empty(var_name: &str) -> Option<String> {
    env::var(var_name).ok()
}
