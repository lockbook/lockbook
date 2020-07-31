use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub writeable_path: String,
}

pub fn dummy_config() -> Config {
    Config {
        writeable_path: "ignored".to_string(),
    }
}

pub struct State {
    pub config: Config,
}
