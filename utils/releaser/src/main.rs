mod android;
mod apple;
mod public_site;
mod secrets;
mod server;
mod utils;
mod windows;

use crate::secrets::*;
use crate::utils::root;

use structopt::StructOpt;

#[derive(StructOpt)]
enum Releaser {
    DeployServer,
    ReleaseApple,
    ReleaseAndroid,
    ReleaseWindows,
    ReleasePublicSite,
}

fn main() {
    root();
    match Releaser::from_args() {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env()),
        Releaser::ReleaseAndroid => android::release_android(&Github::env(), &PlayStore::env()),
        Releaser::ReleaseWindows => windows::release(&Github::env()),
        Releaser::ReleasePublicSite => public_site::release(),
    }
}
