use crate::utils::CommandRunner;
use crate::AppStore;
use std::process::Command;

pub fn release(asc: &AppStore) {
    archive();
    upload(asc);
}

fn archive() {
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
        .assert_success();

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
        .assert_success();
}

fn upload(asc: &AppStore) {
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
        .assert_success();
}
