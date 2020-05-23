use std::env;

pub struct IndexDbConfig<'a> {
    pub user: &'a str,
    pub pass: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub db: &'a str,
    pub cert: &'a str,
}

pub struct FilesDbConfig<'a> {
    pub bucket: &'a str,
    pub region: s3::region::Region,
    pub access_key: &'a str,
    pub secret_key: &'a str,
}

pub struct AuthConfig {
    pub max_auth_delay: u128,
}

pub struct Config<'a> {
    pub index_db_config: IndexDbConfig<'a>,
    pub files_db_config: FilesDbConfig<'a>,
    pub auth_config: AuthConfig,
}

pub fn config<'a>() -> Config<'a> {
    Config {
        index_db_config: IndexDbConfig {
            user: &env::var("INDEX_DB_CONFIG_USER").unwrap(),
            pass: &env::var("INDEX_DB_CONFIG_PASS").unwrap(),
            host: &env::var("INDEX_DB_CONFIG_HOST").unwrap(),
            port: env::var("INDEX_DB_CONFIG_PORT").unwrap().parse().unwrap(),
            db: &env::var("INDEX_DB_CONFIG_DB").unwrap(),
            cert: &env::var("INDEX_DB_CONFIG_CERT").unwrap(),
        },
        files_db_config: FilesDbConfig {
            bucket: &env::var("FILES_DB_CONFIG_BUCKET").unwrap(),
            region: env::var("FILES_DB_CONFIG_REGION").unwrap().parse().unwrap(),
            access_key: &env::var("FILES_DB_CONFIG_ACCESS_KEY").unwrap(),
            secret_key: &env::var("FILES_DB_CONFIG_SECRET_KEY").unwrap(),
        },
        auth_config: AuthConfig {
            max_auth_delay: env::var("MAX_AUTH_DELAY").unwrap().parse().unwrap(),
        },
    }
}
