pub struct Config {
    pub writeable_path: String,
    pub max_auth_delay: i32
}

impl Config {
    pub fn get_auth_delay() -> &'static str {
        env!("MAX_AUTH_DELAY")
    }
}

pub struct State {
    pub config: Config,
}
