use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub logs: bool,
    pub colored_logs: bool,
    pub writeable_path: String,
}
