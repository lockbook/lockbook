use std::env;
use std::fs;

use crate::app_store::AppStoreConfig;

pub struct Config {
    pub github_token: String,
    pub port: u16,
    pub app_store: AppStoreConfig,
}

impl Config {
    pub fn from_env() -> Self {
        let key_path = required("APP_STORE_CONNECT_KEY_PATH");
        let private_key = fs::read_to_string(&key_path)
            .unwrap_or_else(|e| panic!("failed to read {key_path}: {e}"));

        Self {
            github_token: required("GITHUB_TOKEN"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "9898".into())
                .parse()
                .unwrap(),
            app_store: AppStoreConfig {
                issuer_id: required("APP_STORE_CONNECT_ISSUER_ID"),
                key_id: required("APP_STORE_CONNECT_KEY_ID"),
                private_key,
                vendor_number: required("APP_STORE_CONNECT_VENDOR_NUMBER"),
            },
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
