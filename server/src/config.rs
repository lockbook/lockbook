use std::env;

pub struct IndexDbConfig {
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
    pub db: String,
    pub cert: String,
}

pub struct FilesDbConfig {
    pub bucket: String,
    pub region: s3::region::Region,
    pub access_key: String,
    pub secret_key: String,
}

pub struct AuthConfig {
    pub max_auth_delay: u128,
}

pub struct Config {
    pub index_db_config: IndexDbConfig,
    pub files_db_config: FilesDbConfig,
    pub auth_config: AuthConfig,
}

pub fn config() -> Config {
    Config {
        index_db_config: IndexDbConfig {
            user: env::var("INDEX_DB_CONFIG_USER").unwrap(),
            pass: env::var("INDEX_DB_CONFIG_PASS").unwrap(),
            host: env::var("INDEX_DB_CONFIG_HOST").unwrap(),
            port: env::var("INDEX_DB_CONFIG_PORT").unwrap().parse().unwrap(),
            db: env::var("INDEX_DB_CONFIG_DB").unwrap(),
            cert: env::var("INDEX_DB_CONFIG_CERT").unwrap(),
        },
        files_db_config: FilesDbConfig {
            bucket: env::var("FILES_DB_CONFIG_BUCKET").unwrap(),
            region: env::var("FILES_DB_CONFIG_REGION").unwrap().parse().unwrap(),
            access_key: env::var("FILES_DB_CONFIG_ACCESS_KEY").unwrap(),
            secret_key: env::var("FILES_DB_CONFIG_SECRET_KEY").unwrap(),
        },
        auth_config: AuthConfig {
            max_auth_delay: env::var("MAX_AUTH_DELAY").unwrap().parse().unwrap(),
        },
    }
}
