use std::{fs, process::Command};

use cli_rs::cli_error::CliResult;

use crate::{
    places::{root, target, workspace_ffi, workspace_swift_libs},
    utils::CommandRunner,
};

pub fn apple_ws_all() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: true, ios_sim: true, arm_macos: true, x86_macos: true })
}

#[derive(Copy, Clone)]
pub struct WsBuildTargets {
    ios: bool,
    ios_sim: bool,
    arm_macos: bool,
    x86_macos: bool,
}

fn apple_ws(targets: WsBuildTargets) -> CliResult<()> {
    println!("cbindgen");
    fs::remove_dir_all(workspace_swift_libs())?;
    fs::create_dir_all(workspace_ffi().join("include"))?;
    Command::new("cbindgen")
        .args(["-l", "c", "-o", "include/workspace.h"])
        .current_dir(workspace_ffi())
        .assert_success()?;

    // create whatever desired libs
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

    // create universal macOS if desired
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
            workspace_swift_libs(),
        )?;
    } else if targets.x86_macos {
        fs::rename(
            target().join("x86_64-apple-darwin/release/libworkspace.a"),
            workspace_swift_libs(),
        )?;
    }

    println!("building xcframework");
    // -library libworkspace.a -headers ../../include \
    // -library ../../../../../target/aarch64-apple-ios/release/libworkspace.a -headers ../../include \
    // -library ../../../../../target/aarch64-apple-ios-sim/release/libworkspace.a -headers ../../include \
    // -output workspace.xcframework

    let mut xcframework = Command::new("xcodebuild");

    let mut args = vec!["-create-xcframework"];

    if targets.arm_macos || targets.x86_macos {
        args.push("-library");
        args.push("lib");
    }

    Ok(())
}
