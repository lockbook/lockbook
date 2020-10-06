use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub writeable_path: String,
    pub api_url: String,
}

impl Config {
    pub fn path(&self) -> &Path {
        Path::new(&self.writeable_path)
    }
}

pub fn dummy_config() -> Config {
    Config {
        writeable_path: "ignored writable path".to_string(),
        api_url: "ignored api url".to_string(),
    }
}

pub struct State {
    pub config: Config,
}
