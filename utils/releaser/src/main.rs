mod android;
mod apple;
mod secrets;
mod server;
mod utils;

use crate::secrets::{AppStore, Github, PlayStore};
use structopt::StructOpt;

#[derive(StructOpt)]
enum Releaser {
    DeployServer,
    ReleaseApple,
    ReleaseAndroid,
}

fn main() {
    match Releaser::from_args() {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env()),
        Releaser::ReleaseAndroid => android::release_android(&Github::env(), &PlayStore::env()),
    }
}
