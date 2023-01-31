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
use utils::determine_new_version;

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

fn from_args(releaser: Releaser, version: Option<&str>) {
    match releaser {
        Releaser::DeployServer => server::deploy_server(),
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env(), version),
        Releaser::ReleaseAndroid => {
            android::release_android(&Github::env(), &PlayStore::env(), version)
        }
        Releaser::ReleaseWindows => windows::release(&Github::env(), version),
        Releaser::ReleasePublicSite => public_site::release(),
        Releaser::ReleaseLinux => linux::release_linux(version),
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
