mod android;
#[cfg(target_os = "macos")]
mod apple;
mod linux;
mod public_site;
mod secrets;
mod server;
mod utils;
mod windows;

use crate::secrets::*;
use crate::utils::root;

use enum_iterator::{all, Sequence};
use structopt::StructOpt;

#[derive(PartialEq, StructOpt, Sequence)]
enum Releaser {
    All,
    DeployServer,
    #[cfg(target_os = "macos")]
    ReleaseApple,
    ReleaseAndroid,
    ReleaseWindows,
    ReleasePublicSite,
    ReleaseLinux,
}

fn main() {
    // Fail fast if we're invoking from the wrong location
    root();
    from_args(Releaser::from_args());
}

fn from_args(releaser: Releaser) {
    match releaser {
        Releaser::DeployServer => server::deploy_server(),
        #[cfg(target_os = "macos")]
        Releaser::ReleaseApple => apple::release_apple(&Github::env(), &AppStore::env()),
        Releaser::ReleaseAndroid => android::release_android(&Github::env(), &PlayStore::env()),
        Releaser::ReleaseWindows => windows::release(&Github::env()),
        Releaser::ReleasePublicSite => public_site::release(),
        Releaser::ReleaseLinux => linux::release_linux(),
        Releaser::All => {
            for releaser in all::<Releaser>()
                .collect::<Vec<Releaser>>()
                .into_iter()
                .filter(|release| *release != Releaser::All)
            {
                from_args(releaser);
            }
        }
    }
}
