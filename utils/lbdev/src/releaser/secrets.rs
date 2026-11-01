use std::env;

pub struct CratesIO(pub String);
pub struct Github(pub String);
pub struct AppStore(pub String);
pub struct AppStoreConnect {
    pub key_id: String,
    pub issuer_id: String,
    pub private_key: String,
}
pub struct PlayStore {
    pub service_account_key: String,
}
pub struct PartnerCenter {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub product_id: String,
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

impl AppStoreConnect {
    pub fn env() -> Self {
        Self {
            key_id: env_or_panic("APP_STORE_CONNECT_KEY_ID"),
            issuer_id: env_or_panic("APP_STORE_CONNECT_ISSUER_ID"),
            private_key: env_or_panic("APP_STORE_CONNECT_PRIVATE_KEY"),
        }
    }
}

impl PlayStore {
    pub fn env() -> Self {
        Self { service_account_key: env_or_panic("GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY") }
    }
}

impl PartnerCenter {
    pub fn env() -> Self {
        Self {
            tenant_id: env_or_panic("AZURE_AD_TENANT_ID"),
            client_id: env_or_panic("AZURE_AD_APPLICATION_CLIENT_ID"),
            client_secret: env_or_panic("AZURE_AD_APPLICATION_SECRET"),
            product_id: env_or_panic("STORE_PRODUCT_ID"),
        }
    }
}

fn env_or_panic(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("env var: {key} missing"))
}
