mod android;
mod apple;
mod public_site;
mod secrets;
mod server;
mod utils;
mod windows;

use crate::secrets::{AppStore, Github, PlayStore};
use std::env;
use std::path::PathBuf;
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

pub fn root() -> PathBuf {
    let project_root = env::current_dir().unwrap();
    if project_root.file_name().unwrap() != "lockbook" {
        panic!("releaser not called from project root");
    }
    project_root
}
