use std::fs::File;
use std::path::Path;
use std::process::Command;

use gh_release::ReleaseClient;

use crate::{
    secrets::Github,
    utils::{core_version, lb_repo, CommandRunner},
};

pub fn release_installers() {
    build_x86();
}

fn build_x86() {
    let gh = Github::env();
    Command::new("cargo")
        .args(["build", "-p", "lockbook-egui", "--release", "--target=x86_64-pc-windows-msvc"])
        .assert_success();

    Command::new("cargo")
        .env("LB_TARGET", "x86_64-pc-windows-msvc")
        .args(["build", "-p", "winstaller", "--release", "--target=x86_64-pc-windows-msvc"])
        .assert_success();

    upload(
        &gh,
        "lockbook-windows-setup-x86_64.exe",
        "target/x86_64-pc-windows-msvc/release/winstaller.exe",
    );
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
            release.id,
            name,
            "application/vnd.microsoft.portable-executable",
            file,
            None,
        )
        .unwrap();
}
