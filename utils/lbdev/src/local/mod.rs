use std::{fs, path::PathBuf, process::Command};

use cli_rs::cli_error::CliResult;

use crate::{
    places::{apple_dir, local_env_path, root, target, workspace_ffi, workspace_swift_libs},
    utils::CommandRunner,
};

pub fn apple_ws_all() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: true, ios_sim: true, arm_macos: true, x86_macos: true })
}

pub fn apple_ws_macos() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: false, ios_sim: false, arm_macos: true, x86_macos: true })?;
    println!(
        "warning: xcode may need to be restarted if you swap between iOS & macOS and experience build failures"
    );
    Ok(())
}

pub fn apple_ws_ios() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: true, ios_sim: false, arm_macos: false, x86_macos: false })?;
    println!(
        "warning: xcode may need to be restarted if you swap between iOS & macOS and experience build failures"
    );
    Ok(())
}

#[derive(Copy, Clone)]
pub struct WsBuildTargets {
    ios: bool,
    ios_sim: bool,
    arm_macos: bool,
    x86_macos: bool,
}

pub fn android_ws() -> CliResult<()> {
    Command::new("sh")
        .arg("libs/content/workspace-ffi/create_android_libs.sh")
        .current_dir(root())
        .assert_success()?;
    Ok(())
}

fn apple_ws(targets: WsBuildTargets) -> CliResult<()> {
    println!("cbindgen");
    let _ = fs::remove_dir_all(workspace_swift_libs());
    fs::create_dir_all(workspace_ffi().join("include"))?;
    Command::new("cbindgen")
        .args(["-l", "c", "-o", "include/workspace.h"])
        .current_dir(workspace_ffi())
        .assert_success()?;

    let mut ios_build = Command::new("cargo");
    let mut args = vec!["build", "--release"];
    let mut execute_ios = false;

    if targets.ios {
        execute_ios = true;
        args.push("--target=aarch64-apple-ios");
    }

    if targets.ios_sim {
        execute_ios = true;
        args.push("--target=aarch64-apple-ios-sim");
    }

    if execute_ios {
        println!("Building iOS");
        ios_build
            .args(args)
            .current_dir(workspace_ffi())
            .assert_success()?;
    }

    let mut mac_build = Command::new("cargo");
    let mut args = vec!["build", "--release"];
    let mut execute_mac = false;

    if targets.arm_macos {
        execute_mac = true;
        args.push("--target=aarch64-apple-darwin");
    }

    if targets.x86_macos {
        execute_mac = true;
        args.push("--target=x86_64-apple-darwin");
    }

    if execute_mac {
        println!("Building macOS");
        mac_build
            .args(args)
            .current_dir(workspace_ffi())
            .assert_success()?;
    }

    fs::create_dir_all(workspace_swift_libs())?;
    if targets.arm_macos && targets.x86_macos {
        println!("lipoing");
        Command::new("lipo")
            .args([
                "-create",
                "target/aarch64-apple-darwin/release/libworkspace.a",
                "target/x86_64-apple-darwin/release/libworkspace.a",
                "-output",
                workspace_swift_libs()
                    .join("libworkspace.a")
                    .to_str()
                    .unwrap(),
            ])
            .current_dir(root())
            .assert_success()?;
    } else if targets.arm_macos {
        fs::rename(
            target().join("aarch64-apple-darwin/release/libworkspace.a"),
            workspace_swift_libs().join("libworkspace.a"),
        )?;
    } else if targets.x86_macos {
        fs::rename(
            target().join("x86_64-apple-darwin/release/libworkspace.a"),
            workspace_swift_libs().join("libworkspace.a"),
        )?;
    }

    println!("building xcframework");
    let mut xcframework = Command::new("xcodebuild");

    let mut args = vec!["-create-xcframework"];

    if targets.arm_macos || targets.x86_macos {
        args.push("-library");
        args.push("libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    if targets.ios {
        args.push("-library");
        args.push("../../../../../target/aarch64-apple-ios/release/libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    if targets.ios_sim {
        args.push("-library");
        args.push("../../../../../target/aarch64-apple-ios-sim/release/libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    args.push("-output");
    args.push("workspace.xcframework");

    xcframework
        .current_dir(workspace_swift_libs())
        .args(args)
        .assert_success()?;

    Ok(())
}

pub fn apple_run_ios(name: String) -> CliResult<()> {
    let device_id = get_device_id(&name)?;
    apple_ws_ios()?;
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
        .current_dir(root())
        .assert_success()?;

    Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "install",
            "app",
            "--device",
            &device_id,
            "build/Lockbook-iOS.xcarchive/Products/Applications/Lockbook.app/",
        ])
        .current_dir(apple_dir())
        .assert_success()?;

    Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--console",
            "--device",
            &device_id,
            "app.lockbook",
        ])
        .assert_success()?;
    Ok(())
}
pub fn apple_run_macos() -> CliResult<()> {
    apple_ws_macos()?;

    Command::new("xcodebuild")
        .args([
            "-workspace",
            "clients/apple/Lockbook.xcworkspace",
            "-scheme",
            "Lockbook (macOS)",
            "-sdk",
            "macosx",
            "-configuration",
            "Release",
            "-archivePath",
            "clients/apple/build/Lockbook-macOS.xcarchive",
            "archive",
        ])
        .current_dir(root())
        .assert_success()?;

    Command::new("./build/Lockbook-macOS.xcarchive/Products/Applications/Lockbook.app/Contents/MacOS/Lockbook")
        .current_dir(apple_dir())
        .assert_success()?;

    Ok(())
}

pub fn apple_device_name_completor(prompt: &str) -> CliResult<Vec<String>> {
    Ok(devices()?
        .into_iter()
        .filter(|entry| entry.starts_with(prompt))
        .collect())
}

pub fn get_device_id(name: &str) -> CliResult<String> {
    Ok(devices_and_ids()?
        .into_iter()
        .find(|row| row.0 == name)
        .unwrap()
        .1)
}

fn devices() -> CliResult<Vec<String>> {
    Ok(devices_and_ids()?.into_iter().map(|a| a.0).collect())
}

fn devices_and_ids() -> CliResult<Vec<(String, String)>> {
    let json_path_str = "/tmp/devicectl_devices.json";
    let json_path = PathBuf::from(json_path_str);

    Command::new("xcrun")
        .args([
            "devicectl",
            "list",
            "devices",
            "--hide-default-columns",
            "--filter",
            "State BEGINSWITH 'available'",
            "-q",
            "-j",
            json_path_str,
        ])
        .assert_success()?;

    // Read and parse JSON
    let json_str = fs::read_to_string(&json_path)?;
    let root: Root = serde_json::from_str(&json_str)?;

    // Map to Vec<(name, identifier)>
    Ok(root
        .result
        .devices
        .into_iter()
        .map(|d| (d.device_properties.name, d.identifier))
        .collect())
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Root {
    result: ResultField,
}

#[derive(Debug, Deserialize)]
struct ResultField {
    devices: Vec<Device>,
}

#[derive(Debug, Deserialize)]
struct Device {
    identifier: String,

    #[serde(rename = "deviceProperties")]
    device_properties: DeviceProperties,
}

#[derive(Debug, Deserialize)]
struct DeviceProperties {
    name: String,
}

pub fn server() -> CliResult<()> {
    dotenvy::from_path(local_env_path()).unwrap();
    Command::new("cargo")
        .args(["run", "-p", "lockbook-server", "--release"])
        .current_dir(root())
        .assert_success()?;

    Ok(())
}
