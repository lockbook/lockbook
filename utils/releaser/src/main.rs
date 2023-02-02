mod android;
mod apple;
mod linux;
mod public_site;
mod secrets;
mod server;
mod utils;
mod windows;

use crate::secrets::*;
use crate::utils::root;

use structopt::StructOpt;
use utils::bump_versions;

#[derive(PartialEq, StructOpt)]
#[structopt(name = "basic")]
enum Releaser {
    All,
    DeployServer,
    ReleaseApple,
    ReleaseAndroid,
    ReleaseWindows,
    ReleasePublicSite,
    ReleaseLinux,
    BumpVersion {
        #[structopt(short, long, name = "bump type")]
        increment: Option<String>,
    },
}

fn main() {
    // Fail fast if we're invoking from the wrong location
    root();

    from_args(Releaser::from_args());
}

fn from_args(releaser: Releaser) {
    match releaser {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env()),
        Releaser::ReleaseAndroid => android::release_android(&Github::env(), &PlayStore::env()),
        Releaser::ReleaseWindows => windows::release(&Github::env()),
        Releaser::ReleasePublicSite => public_site::release(),
        Releaser::ReleaseLinux => linux::release_linux(),
        Releaser::BumpVersion { increment } => bump_versions(increment),
        Releaser::All => {
            let releases = if cfg!(target_os = "macos") {
                vec![Releaser::ReleaseApple]
            } else if cfg!(target_os = "linux") {
                vec![
                    Releaser::DeployServer,
                    Releaser::ReleaseLinux,
                    Releaser::ReleaseAndroid,
                    Releaser::ReleasePublicSite,
                ]
            } else {
                vec![Releaser::ReleaseWindows]
            };

            for releaser in releases {
                from_args(releaser);
            }
        }
    }
}
