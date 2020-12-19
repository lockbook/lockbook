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

pub fn temp_config() -> Config {
    Config {
        writeable_path: String::from(tempfile::tempdir().unwrap().path().to_str().unwrap()),
    }
}

pub struct State {
    pub config: Config,
}
