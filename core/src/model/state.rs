use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub writeable_path: String,
}

// TODO remove
pub fn temp_config() -> Config {
    Config { writeable_path: String::from(tempfile::tempdir().unwrap().path().to_str().unwrap()) }
}
