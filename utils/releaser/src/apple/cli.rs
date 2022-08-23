use crate::utils::{core_version, lb_repo, sha_file, CommandRunner};
use crate::Github;
use gh_release::ReleaseClient;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

static CLI_NAME: &str = "lockbook-cli-macos.tar.gz";

pub fn release(gh: &Github) {
    build_x86();
    build_arm();
    lipo_binaries();
    tar_binary();
    upload(gh);
    update_brew();
}

fn build_x86() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-cli", "--release", "--target=x86_64-apple-darwin"])
        .assert_success();
}

fn build_arm() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-cli", "--release", "--target=aarch64-apple-darwin"])
        .assert_success();
}

fn lipo_binaries() {
    fs::create_dir_all("target/universal-cli/").unwrap();
    Command::new("lipo")
        .args([
            "-create",
            "-output",
            "target/universal-cli/lockbook",
            "target/x86_64-apple-darwin/release/lockbook",
            "target/aarch64-apple-darwin/release/lockbook",
        ])
        .assert_success();
}

fn tar_binary() {
    Command::new("tar")
        .args(["-czf", CLI_NAME, "lockbook"])
        .current_dir("target/universal-cli")
        .assert_success();
}

fn tarred_binary() -> String {
    format!("target/universal-cli/{CLI_NAME}")
}

fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open(tarred_binary()).unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id as u64,
            "lockbook-cli-macos.tar.gz",
            "application/gzip",
            file,
            None,
        )
        .unwrap();
}

fn update_brew() {
    overwrite_lockbook_rb();
    push_brew();
}

fn overwrite_lockbook_rb() {
    let version = core_version();
    let sha = sha_file(&tarred_binary());

    let new_content = format!(
        r#"
class Lockbook < Formula
  desc "The best place to store and share thoughts."
  homepage "https://github.com/lockbook/lockbook"
  url "https://github.com/lockbook/lockbook/releases/download/{version}/{CLI_NAME}"
  sha256 "{sha}"
  version "{version}"

  def install
    bin.install "lockbook"
  end
end
"#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../homebrew-lockbook/Formula/lockbook.rb")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();
}

fn push_brew() {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir("../homebrew-lockbook")
        .assert_success();
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir("../homebrew-lockbook")
        .assert_success();
    Command::new("git")
        .args(["push", "origin", "master"])
        .current_dir("../homebrew-lockbook")
        .assert_success();
}
