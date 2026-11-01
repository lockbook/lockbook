pub mod app_store;
pub mod cli;
pub mod ios;
pub mod mac;

use chrono_tz::America::New_York;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use google_androidpublisher3::chrono::Utc;

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::local::apple_ws_all;
use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;

pub fn release() -> CliResult<()> {
    cli::release()?;
    apple_ws_all()?;
    clean_build_dir();
    ios::release(false)?;
    mac::release(false, true, true)?;
    upload_dsyms()?;
    app_store::submit_for_review()?;
    Ok(())
}

fn upload_dsyms() -> CliResult<()> {
    let client = ReleaseClient::new(Github::env().0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();

    for (platform, name) in
        [("iOS", "lockbook-ios-dsyms.zip"), ("macOS", "lockbook-macos-dsyms.zip")]
    {
        let symbols = format!("clients/apple/build/Lockbook-{platform}.xcarchive/dSYMs");
        let archive = format!("clients/apple/build/{name}");
        Command::new("ditto")
            .args(["-c", "-k", "--keepParent", &symbols, &archive])
            .assert_success()?;
        client
            .upload_release_asset(
                &lb_repo(),
                release.id,
                name,
                "application/zip",
                fs::File::open(&archive).unwrap(),
                None,
            )
            .unwrap();
    }
    Ok(())
}

pub fn build_number() -> String {
    Utc::now()
        .with_timezone(&New_York)
        .format("%Y%m%d.%H%M%S")
        .to_string()
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
