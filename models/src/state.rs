use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub writeable_path: String,
}

impl Config {
    pub fn path(&self) -> &Path {
        Path::new(&self.writeable_path)
    }
}

pub fn dummy_config() -> Config {
    Config {
        writeable_path: "ignored".to_string(),
    }
}

pub struct State {
    pub config: Config,
}
