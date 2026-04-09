use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;

use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;

pub fn release() -> CliResult<()> {
    build()?;
    zip_binary("target/release/lockbook.exe", "windows-build/lockbook-cli.zip")?;
    upload("windows-build/lockbook-cli.zip", "lockbook-windows-cli.zip")?;
    Ok(())
}

pub fn release_arm() -> CliResult<()> {
    build_arm()?;
    zip_binary(
        "target/aarch64-pc-windows-msvc/release/lockbook.exe",
        "windows-build/lockbook-cli-arm.zip",
    )?;
    upload("windows-build/lockbook-cli-arm.zip", "lockbook-windows-cli-arm.zip")?;
    Ok(())
}

fn build() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release"])
        .assert_success()
}

fn build_arm() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release", "--target=aarch64-pc-windows-msvc"])
        .assert_success()
}

fn zip_binary(exe_path: &str, zip_path: &str) -> CliResult<()> {
    let exe_bytes = fs::read(exe_path).unwrap();

    let zip_file = File::create(zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(zip_file);

    zip.start_file("lockbook.exe", Default::default()).unwrap();
    zip.write_all(&exe_bytes).unwrap();
    zip.finish().unwrap();
    Ok(())
}

fn upload(zip_path: &str, asset_name: &str) -> CliResult<()> {
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();
    let file = File::open(zip_path).unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            asset_name,
            "application/zip",
            file,
            None,
        )
        .unwrap();
    Ok(())
}
