use std::env;

use crate::app_store::AppStoreConfig;

pub struct Config {
    pub github_token: String,
    pub port: u16,
    pub app_store: AppStoreConfig,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            github_token: required("GITHUB_TOKEN"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "9898".into())
                .parse()
                .unwrap(),
            app_store: AppStoreConfig {
                issuer_id: required("APP_STORE_CONNECT_ISSUER_ID"),
                key_id: required("APP_STORE_CONNECT_KEY_ID"),
                private_key: required("APP_STORE_CONNECT_PRIVATE_KEY"),
                vendor_number: required("APP_STORE_CONNECT_VENDOR_NUMBER"),
            },
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
