use std::env;
use std::time::Duration;

#[derive(Clone)]
pub struct IndexDbConf {
    pub redis_url: String,
}

impl IndexDbConf {
    pub fn from_env_vars() -> Self {
        Self {
            redis_url: env_or_panic("INDEX_DB_REDIS_URL"),
        }
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
pub struct ServerConfig {
    pub port: u16,
    pub max_auth_delay: u128,
    pub log_path: String,
    pub pd_api_key: Option<String>,
    pub ssl_cert_location: Option<String>,
    pub ssl_private_key_location: Option<String>,
}

impl ServerConfig {
    pub fn from_env_vars() -> ServerConfig {
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
    pub duration_between_metrics_refresh: Duration,
    pub duration_between_user_metrics: Duration,
    pub duration_between_getting_pub_key_metrics: Duration,
    pub duration_between_getting_pub_key_key_metrics: Duration,
}

impl MetricsConfig {
    pub fn from_env_vars() -> MetricsConfig {
        MetricsConfig {
            duration_between_metrics_refresh: Duration::from_secs(
                env_or_panic("MINUTES_BETWEEN_METRICS_REFRESH")
                    .parse::<u64>()
                    .unwrap()
                    * 60,
            ),
            duration_between_user_metrics: Duration::from_millis(
                env_or_panic("MILLIS_BETWEEN_USER_METRICS")
                    .parse::<u64>()
                    .unwrap(),
            ),
            duration_between_getting_pub_key_metrics: Duration::from_millis(
                env_or_panic("MILLIS_BETWEEN_GETTING_PUB_KEY_METRICS")
                    .parse::<u64>()
                    .unwrap(),
            ),
            duration_between_getting_pub_key_key_metrics: Duration::from_millis(
                env_or_panic("MILLIS_BETWEEN_GETTING_PUB_KEY_KEY_METRICS")
                    .parse::<u64>()
                    .unwrap(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub index_db: IndexDbConf,
    pub files_db: FilesDbConfig,
    pub server: ServerConfig,
    pub metrics: MetricsConfig,
}

impl Config {
    pub fn from_env_vars() -> Self {
        Self {
            index_db: IndexDbConf::from_env_vars(),
            files_db: FilesDbConfig::from_env_vars(),
            server: ServerConfig::from_env_vars(),
            metrics: MetricsConfig::from_env_vars(),
        }
    }
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).expect(&format!("Missing environment variable {}", var_name))
}

fn env_or_empty(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(var) => Some(var),
        Err(_) => None,
    }
}
