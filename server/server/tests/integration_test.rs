#![allow(dead_code)]

use lockbook_core::model::state::Config;

use std::env;
use uuid::Uuid;

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4().to_string()),
    }
}

pub fn api_url() -> String {
    env::var("API_URL").expect("API_URL must be defined!")
}

pub fn random_uuid() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
