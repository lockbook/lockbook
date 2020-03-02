use openssl::pkey::Private;
use openssl::rsa::Rsa;

pub struct Account {
    username: String,
    public_key: Rsa<Private>,
}

pub struct Config {
    pub writeable_path: String,
}

pub struct State {
    pub config: Config,
}