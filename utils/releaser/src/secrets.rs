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
        Self(env_or_panic("GITHUB_TOKEN"))
    }
}

impl AppStore {
    pub fn env() -> Self {
        Self(env_or_panic("APPLE_ID_PASSWORD"))
    }
}

impl PlayStore {
    pub fn env() -> Self {
        Self {
            service_account_key: env_or_panic("GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY"),
            release_store_file: env_or_panic("ANDROID_RELEASE_STORE_FILE"),
            release_store_password: env_or_panic("ANDROID_RELEASE_STORE_PASSWORD"),
            release_key_alias: env_or_panic("ANDROID_RELEASE_KEY_ALIAS"),
            release_key_password: env_or_panic("ANDROID_RELEASE_KEY_PASSWORD"),
        }
    }
}

fn env_or_panic(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("env var: {key} missing"))
}
