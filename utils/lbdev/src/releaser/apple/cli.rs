use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version, sha_file};
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

static CLI_NAME: &str = "lockbook-cli-macos.tar.gz";

pub fn release() -> CliResult<()> {
    build()?;
    upload();
    update_brew()?;

    Ok(())
}

pub fn build() -> CliResult<()> {
    build_x86()?;
    build_arm()?;
    lipo_binaries()?;
    tar_binary()?;

    Ok(())
}

fn build_x86() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release", "--target=x86_64-apple-darwin"])
        .assert_success()
}

fn build_arm() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release", "--target=aarch64-apple-darwin"])
        .assert_success()
}

fn lipo_binaries() -> CliResult<()> {
    fs::create_dir_all("target/universal-cli/").unwrap();
    Command::new("lipo")
        .args([
            "-create",
            "-output",
            "target/universal-cli/lockbook",
            "target/x86_64-apple-darwin/release/lockbook",
            "target/aarch64-apple-darwin/release/lockbook",
        ])
        .assert_success()
}

fn tar_binary() -> CliResult<()> {
    Command::new("tar")
        .args(["-czf", CLI_NAME, "lockbook"])
        .current_dir("target/universal-cli")
        .assert_success()
}

fn tarred_binary() -> String {
    format!("target/universal-cli/{CLI_NAME}")
}

fn upload() {
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();
    let file = File::open(tarred_binary()).unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-cli-macos.tar.gz",
            "application/gzip",
            file,
            None,
        )
        .unwrap();
}

fn update_brew() -> CliResult<()> {
    let gh = Github::env();
    let temp_dir = env::temp_dir().join("homebrew-lockbook");

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    clone_brew_repo(&gh, &temp_dir)?;
    overwrite_lockbook_rb(&temp_dir);
    push_brew(&temp_dir)?;

    fs::remove_dir_all(&temp_dir).unwrap();
    Ok(())
}

fn clone_brew_repo(gh: &Github, temp_dir: &Path) -> CliResult<()> {
    let repo_url = format!("https://parth:{}@github.com/lockbook/homebrew-lockbook.git", gh.0);
    Command::new("git")
        .args(["clone", &repo_url, temp_dir.to_str().unwrap()])
        .assert_success()
}

fn overwrite_lockbook_rb(temp_dir: &Path) {
    let version = lb_version();
    let sha = sha_file(&tarred_binary());

    let new_content = format!(
        r#"
class Lockbook < Formula
  desc "The private, polished note-taking platform."
  homepage "https://github.com/lockbook/lockbook"
  url "https://github.com/lockbook/lockbook/releases/download/{version}/{CLI_NAME}"
  sha256 "{sha}"
  version "{version}"

  def install
    bin.install "lockbook"
    generate_completions_from_executable(bin/"lockbook", "completions")
  end
  def caveats
    <<~EOS
      If you haven't already, enable completions for binaries installed by brew: #{{Formatter.url("https://docs.brew.sh/Shell-Completion")}}
    EOS
  end
end
"#
    );

    let formula_path = temp_dir.join("Formula/lockbook.rb");
    let mut file = File::create(formula_path).unwrap();
    file.write_all(new_content.as_bytes()).unwrap();
}

fn push_brew(temp_dir: &Path) -> CliResult<()> {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(temp_dir)
        .assert_success()?;
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir(temp_dir)
        .assert_success()?;
    Command::new("git")
        .args(["push", "origin", "master"])
        .current_dir(temp_dir)
        .assert_success()?;
    Ok(())
}
