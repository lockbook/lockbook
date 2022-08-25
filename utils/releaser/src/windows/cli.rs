use std::fs::File;
use std::process::Command;

use gh_release::ReleaseClient;

use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;

pub fn release(gh: &Github) {
    build();
    upload(gh);
}

fn build() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-cli", "--release"])
        .assert_success();
}

fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open("target/release/lockbook.exe").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id as u64,
            "lockbook.exe",
            "application/octet-stream",
            file,
            None,
        )
        .unwrap();
}

//fn zip_binary() {}
