mod apple;
mod secrets;
mod server;
mod utils;
mod windows;

use crate::secrets::{AppStore, Github};
use structopt::StructOpt;

#[derive(StructOpt)]
enum Releaser {
    DeployServer,
    ReleaseApple,
    ReleaseWindows,
}

fn main() {
    match Releaser::from_args() {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env()),
        Releaser::ReleaseWindows => windows::release(&Github::env()),
    }
}
