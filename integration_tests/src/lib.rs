use lockbook_core::model::state::Config;
use lockbook_core::repo::db_provider::{DbProvider, TempBackedDB};
use lockbook_core::Db;
use uuid::Uuid;

mod account_tests;

pub fn test_db() -> Db {
    let config = Config {
        writeable_path: "ignored".to_string(),
    };
    TempBackedDB::connect_to_db(&config).unwrap()
}

pub fn random_username() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
