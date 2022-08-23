use std::env;

pub struct Github(pub String);
pub struct AppStore(pub String);

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
