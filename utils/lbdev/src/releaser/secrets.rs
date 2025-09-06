use std::env;

pub struct CratesIO(pub String);
pub struct Github(pub String);
pub struct AppStore(pub String);
pub struct PlayStore {
    pub service_account_key: String,
}

impl CratesIO {
    pub fn env() -> Self {
        Self(env_or_panic("CRATES_IO_API_TOKEN"))
    }
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
        Self { service_account_key: env_or_panic("GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY") }
    }
}

fn env_or_panic(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("env var: {key} missing"))
}
