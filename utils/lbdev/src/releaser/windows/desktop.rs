use std::fs::File;
use std::path::Path;
use std::process::Command;

use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;

use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;

pub fn release() -> CliResult<()> {
    let gh = Github::env();
    build_x86()?;
    upload(
        &gh,
        "lockbook-windows-setup-x86_64.exe",
        "target/x86_64-pc-windows-msvc/release/winstaller.exe",
    )?;

    Ok(())
}

fn build_x86() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-windows", "--release", "--target=x86_64-pc-windows-msvc"])
        .assert_success()?;

    Command::new("cargo")
        .env("LB_TARGET", "x86_64-pc-windows-msvc")
        .args([
            "build",
            "-p",
            "winstaller",
            "--release",
            "--target=x86_64-pc-windows-msvc",
            "--features",
            "build-winstaller",
        ])
        .assert_success()?;

    Ok(())
}

fn upload<P: AsRef<Path>>(gh: &Github, name: &str, fpath: P) -> CliResult<()> {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();

    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
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

    Ok(())
}
