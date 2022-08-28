use std::env;

pub struct Github(pub String);
pub struct AppStore(pub String);
pub struct PlayStore {
    pub service_account_key: String,
    pub release_store_file: String,
    pub release_store_password: String,
    pub release_key_alias: String,
    pub release_key_password: String,
}

impl Github {
    pub fn env() -> Self {
        Self(env::var("GITHUB_TOKEN").unwrap())
    }
}

impl AppStore {
    pub fn env() -> Self {
        Self(env::var("APPLE_ID_PASSWORD").unwrap())
    }
}

impl PlayStore {
    pub fn env() -> Self {
        Self {
            service_account_key: env::var("GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY").unwrap(),
            release_store_file: env::var("ANDROID_RELEASE_STORE_FILE").unwrap(),
            release_store_password: env::var("ANDROID_RELEASE_STORE_PASSWORD").unwrap(),
            release_key_alias: env::var("ANDROID_RELEASE_KEY_ALIAS").unwrap(),
            release_key_password: env::var("ANDROID_RELEASE_KEY_PASSWORD").unwrap(),
        }
    }
}
