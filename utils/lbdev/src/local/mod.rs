use std::{fs, process::Command};

use cli_rs::cli_error::CliResult;

use crate::{places::workspace_ffi, utils::CommandRunner};

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
    // cbindgen
    fs::remove_dir_all(workspace_ffi().join("SwiftWorkspace/Libs"))?;
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
    if targets.arm_macos && targets.x86_macos {}

    Ok(())
}
