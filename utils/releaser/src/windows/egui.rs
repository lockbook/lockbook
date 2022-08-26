use std::fs::{self, File};
use std::process::Command;

use gh_release::ReleaseClient;

use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;

pub fn release_installers(gh: &Github) {
    build_x86();
    build_arm();
    upload(gh);
}

fn build_x86(gh: &Github) {
    Command::new("cargo")
        .env("LB_TARGET", "x86_64-pc-windows-msvc")
        .args(["build", "-p", "winstaller", "--release", "--target=x86_64-pc-windows-msvc"])
        .assert_success();

	upload(gh, "lockbook-windows-setup-x86_64.exe");
}

fn build_arm(gh: &Github) {
    Command::new("cargo")
		.env("LB_TARGET", "aarch64-pc-windows-msvc")
        .args(["build", "-p", "winstaller", "--release", "--target=aarch64-pc-windows-msvc"])
        .assert_success();

	upload(gh, "lockbook-windows-setup-aarch64.exe");
}

fn upload<P: AsRef<Path>>(gh: &Github, name: &str, fpath: P) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();

    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();

    let file = File::open(fpath).unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id as u64,
            name,
            "application/vnd.microsoft.portable-executable",
            file,
            None,
        )
        .unwrap();
}
