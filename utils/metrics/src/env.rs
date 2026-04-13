use std::env;
use std::fs;
use std::path::PathBuf;

use crate::app_store::AppStoreConfig;
use crate::play_store::PlayStoreConfig;

pub struct Config {
    pub github_token: String,
    pub port: u16,
    pub app_store: AppStoreConfig,
    pub play_store: PlayStoreConfig,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        let key_path = required("APP_STORE_CONNECT_KEY_PATH");
        let private_key = fs::read_to_string(&key_path)
            .unwrap_or_else(|e| panic!("failed to read {key_path}: {e}"));

        let data_dir = env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/parth/metrics-data"));

        let play_store_key_path = required("PLAY_STORE_SERVICE_ACCOUNT_KEY_PATH");
        let play_store_key = fs::read_to_string(&play_store_key_path)
            .unwrap_or_else(|e| panic!("failed to read {play_store_key_path}: {e}"));

        Self {
            github_token: required("GITHUB_TOKEN"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8081".into())
                .parse()
                .unwrap(),
            app_store: AppStoreConfig {
                issuer_id: required("APP_STORE_CONNECT_ISSUER_ID"),
                key_id: required("APP_STORE_CONNECT_KEY_ID"),
                private_key,
                vendor_number: required("APP_STORE_CONNECT_VENDOR_NUMBER"),
            },
            play_store: PlayStoreConfig {
                service_account_key: play_store_key,
                bucket: required("PLAY_STORE_BUCKET"),
            },
            data_dir,
        }
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
