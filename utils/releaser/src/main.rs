mod server;
mod utils;

use structopt::StructOpt;

#[derive(StructOpt)]
enum Releaser {
    DeployServer,
}

fn main() {
    match Releaser::from_args() {
        Releaser::DeployServer => server::deploy_server(),
    }
}
