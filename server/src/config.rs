use std::sync::Mutex;

pub struct ServerState {
    pub index_db_client: Mutex<postgres::Client>,
    pub files_db_client: Mutex<s3::bucket::Bucket>,
}

pub struct IndexDbConfig {
    pub user: &'static str,
    pub pass: &'static str,
    pub host: &'static str,
    pub port: &'static str,
    pub db: &'static str,
    pub cert: &'static str,
}

pub struct FilesDbConfig {
    pub bucket: &'static str,
    pub region: &'static str,
    pub access_key: &'static str,
    pub secret_key: &'static str,
}

pub struct AuthConfig {
    pub max_auth_delay: &'static str,
}

pub struct Config {
    pub index_db_config: IndexDbConfig,
    pub files_db_config: FilesDbConfig,
    pub auth_config: AuthConfig,
}

pub fn config() -> Config {
    Config {
        index_db_config: IndexDbConfig {
            user: env!("INDEX_DB_CONFIG_USER"),
            pass: env!("INDEX_DB_CONFIG_PASS"),
            host: env!("INDEX_DB_CONFIG_HOST"),
            port: env!("INDEX_DB_CONFIG_PORT"),
            db: env!("INDEX_DB_CONFIG_DB"),
            cert: env!("INDEX_DB_CONFIG_CERT"),
        },
        files_db_config: FilesDbConfig {
            bucket: env!("FILES_DB_CONFIG_BUCKET"),
            region: env!("FILES_DB_CONFIG_REGION"),
            access_key: env!("FILES_DB_CONFIG_ACCESS_KEY"),
            secret_key: env!("FILES_DB_CONFIG_SECRET_KEY"),
        },
        auth_config: AuthConfig {
            max_auth_delay: env!("MAX_AUTH_DELAY"),
        },
    }
}
