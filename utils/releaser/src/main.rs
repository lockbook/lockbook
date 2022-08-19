mod apple;
mod server;
mod utils;

use std::env;
use structopt::StructOpt;

#[derive(StructOpt)]
enum Releaser {
    DeployServer,
    ReleaseApple,
}

pub struct Secrets {
    pub gh_token: String,
}

impl Secrets {
    fn from_env_vars() -> Self {
        let gh_token = env::var("GITHUB_TOKEN").unwrap();
        Self { gh_token }
    }
}

fn main() {
    match Releaser::from_args() {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(Secrets::from_env_vars()),
    }
}
