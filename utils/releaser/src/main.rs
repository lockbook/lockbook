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
use utils::{core_version, determine_new_version};

#[derive(PartialEq, StructOpt)]
#[structopt(name = "basic")]
enum Releaser {
    All {
        // significance of bump. can be major, minor or patch
        #[structopt(short, long)]
        version_bump: Option<String>,
    },
    DeployServer,
    ReleaseApple,
    ReleaseAndroid,
    ReleaseWindows,
    ReleasePublicSite,
    ReleaseLinux,
}

fn main() {
    // Fail fast if we're invoking from the wrong location
    root();

    from_args(Releaser::from_args(), None);
}

fn from_args(releaser: Releaser, new_version: Option<&str>) {
    let current_version = core_version();
    let new_version = new_version.unwrap_or(&current_version);

    match releaser {
        Releaser::DeployServer => server::deploy_server(new_version),
        Releaser::ReleaseApple => {
            apple::release_apple(&Github::env(), &AppStore::env(), new_version)
        }
        Releaser::ReleaseAndroid => {
            android::release_android(&Github::env(), &PlayStore::env(), new_version)
        }
        Releaser::ReleaseWindows => windows::release(&Github::env(), new_version),
        Releaser::ReleasePublicSite => public_site::release(),
        Releaser::ReleaseLinux => linux::release_linux(new_version),
        Releaser::All { version_bump } => {
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

            let new_version = determine_new_version(version_bump);

            for releaser in releases {
                from_args(releaser, new_version.as_deref());
            }
        }
    }
}
