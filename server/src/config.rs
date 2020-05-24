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
    pub host: String,
    pub port: u16,
    pub region: String,
    pub bucket: String,
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
            host: env_or_panic("INDEX_DB_HOST"),
            port: env_or_panic("INDEX_DB_PORT").parse().unwrap(),
            db: env_or_panic("INDEX_DB_DB"),
            user: env_or_panic("INDEX_DB_USER"),
            pass: env_or_panic("INDEX_DB_PASS"),
            cert: env_or_panic("INDEX_DB_CERT"),
        },
        files_db_config: FilesDbConfig {
            host: env_or_panic("FILES_DB_HOST"),
            port: env_or_panic("FILES_DB_PORT").parse().unwrap(),
            region: env_or_panic("FILES_DB_REGION").parse().unwrap(),
            bucket: env_or_panic("FILES_DB_BUCKET"),
            access_key: env_or_panic("FILES_DB_ACCESS_KEY"),
            secret_key: env_or_panic("FILES_DB_SECRET_KEY"),
        },
        auth_config: AuthConfig {
            max_auth_delay: env_or_panic("MAX_AUTH_DELAY").parse().unwrap(),
        },
    }
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).expect(&format!("Missing environment variable {}", var_name))
}
