use std::env;

#[derive(Clone)]
pub struct IndexDbConfig {
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
    pub db: String,
    pub cert: String,
    pub pool_size: u32,
}

impl IndexDbConfig {
    pub fn from_env_vars() -> IndexDbConfig {
        IndexDbConfig {
            host: env_or_panic("INDEX_DB_HOST"),
            port: env_or_panic("INDEX_DB_PORT").parse().unwrap(),
            db: env_or_panic("INDEX_DB_DB"),
            user: env_or_panic("INDEX_DB_USER"),
            pass: env_or_panic("INDEX_DB_PASS"),
            cert: env_or_panic("INDEX_DB_CERT"),
            pool_size: env_or_panic("INDEX_DB_POOL_SIZE").parse().unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct FilesDbConfig {
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl FilesDbConfig {
    pub fn from_env_vars() -> FilesDbConfig {
        FilesDbConfig {
            scheme: env_or_empty("FILES_DB_SCHEME"),
            host: env_or_empty("FILES_DB_HOST"),
            port: env_or_empty("FILES_DB_PORT").map(|e| e.parse().expect("Expected u16!")),
            region: env_or_panic("FILES_DB_REGION").parse().unwrap(),
            bucket: env_or_panic("FILES_DB_BUCKET"),
            access_key: env_or_panic("FILES_DB_ACCESS_KEY"),
            secret_key: env_or_panic("FILES_DB_SECRET_KEY"),
        }
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub max_auth_delay: u128,
    pub log_path: String,
    pub pd_api_key: Option<String>,
}

impl ServerConfig {
    pub fn from_env_vars() -> ServerConfig {
        ServerConfig {
            port: env_or_panic("SERVER_PORT").parse().unwrap(),
            max_auth_delay: env_or_panic("MAX_AUTH_DELAY").parse().unwrap(),
            log_path: env_or_panic("LOG_PATH").parse().unwrap(),
            pd_api_key: env_or_empty("PD_KEY"),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub build: String,
    pub index_db: IndexDbConfig,
    pub files_db: FilesDbConfig,
    pub server: ServerConfig,
}

impl Config {
    pub fn from_env_vars() -> Config {
        Config {
            build: String::from(env!("CARGO_PKG_VERSION")),
            index_db: IndexDbConfig::from_env_vars(),
            files_db: FilesDbConfig::from_env_vars(),
            server: ServerConfig::from_env_vars(),
        }
    }
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).expect(&format!("Missing environment variable {}", var_name))
}

fn env_or_empty(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(var) => Some(var),
        Err(_) => None,
    }
}
