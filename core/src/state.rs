pub struct Config {
    pub writeable_path: String,
    pub max_auth_delay: i32
}

pub struct State {
    pub config: Config,
}
