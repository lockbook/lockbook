use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;

use crate::releaser::secrets::Github;
use crate::releaser::utils::{CommandRunner, lb_repo, lb_version};

pub fn release() -> CliResult<()> {
    build()?;
    zip_binary()?;
    upload()?;
    Ok(())
}

fn build() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release"])
        .assert_success();
    Ok(())
}

fn zip_binary() -> CliResult<()> {
    let exe_bytes = fs::read("target/release/lockbook.exe").unwrap();

    let zip_file = File::create("windows-build/lockbook-cli.zip").unwrap();
    let mut zip = zip::ZipWriter::new(zip_file);

    zip.start_file("lockbook.exe", Default::default()).unwrap();
    zip.write_all(&exe_bytes).unwrap();
    zip.finish().unwrap();
    Ok(())
}

fn upload() -> CliResult<()> {
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
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
    Ok(())
}
