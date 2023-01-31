use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

use gh_release::ReleaseClient;

use crate::utils::{core_version, edit_cargo_version, lb_repo, CommandRunner};
use crate::Github;

pub fn release(gh: &Github, version: &str) {
    edit_cargo_version("clients/cli/", version);
    build();
    zip_binary();
    upload(gh);
}

fn build() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-cli", "--release"])
        .assert_success();
}

fn zip_binary() {
    let exe_bytes = fs::read("target/release/lockbook.exe").unwrap();

    let zip_file = File::create("windows-build/lockbook-cli.zip").unwrap();
    let mut zip = zip::ZipWriter::new(zip_file);

    zip.start_file("lockbook.exe", Default::default()).unwrap();
    zip.write_all(&exe_bytes).unwrap();
    zip.finish().unwrap();
}

fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open("windows-build/lockbook-cli.zip").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-windows-cli.zip",
            "application/zip",
            file,
            None,
        )
        .unwrap();
}
