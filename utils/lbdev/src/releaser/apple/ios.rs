use cli_rs::cli_error::CliResult;

use crate::local::apple_ws_ios;
use crate::releaser::secrets::AppStore;
use crate::utils::CommandRunner;
use std::process::Command;

use super::clean_build_dir;

pub fn release(clean_and_build: bool) -> CliResult<()> {
    if clean_and_build {
        apple_ws_ios()?;
        clean_build_dir();
    }
    archive()?;
    upload()?;
    Ok(())
}

fn archive() -> CliResult<()> {
    Command::new("xcodebuild")
        .args([
            "-workspace",
            "clients/apple/Lockbook.xcworkspace",
            "-scheme",
            "Lockbook (iOS)",
            "-sdk",
            "iphoneos",
            "-configuration",
            "Release",
            "-archivePath",
            "clients/apple/build/Lockbook-iOS.xcarchive",
            "archive",
        ])
        .assert_success()?;

    Command::new("xcodebuild")
        .args([
            "-allowProvisioningUpdates",
            "-archivePath",
            "clients/apple/build/Lockbook-iOS.xcarchive",
            "-exportPath",
            "clients/apple/build",
            "-exportOptionsPlist",
            "clients/apple/exportOptions.plist",
            "-exportArchive",
        ])
        .assert_success()?;
    Ok(())
}

fn upload() -> CliResult<()> {
    let asc = AppStore::env();
    Command::new("xcrun")
        .args([
            "altool",
            "--upload-app",
            "-t",
            "ios",
            "-f",
            "clients/apple/build/lockbook.ipa",
            "-u",
            "parth@mehrotra.me",
            "-p",
            &asc.0,
        ])
        .assert_success()
}
