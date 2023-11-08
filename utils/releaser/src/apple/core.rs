use crate::utils::CommandRunner;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

static INC: &str = "clients/apple/CLockbookCore/Sources/CLockbookCore/include/";
static IOS_LIB_DIR: &str = "clients/apple/CLockbookCore/Sources/CLockbookCore/lib_ios/";
static MAC_LIB_DIR: &str = "clients/apple/CLockbookCore/Sources/CLockbookCore/lib/";
static LIB: &str = "liblb_external_interface.a";
static HEAD: &str = "lb_rs.h";

pub fn build() {
    clean_dirs();
    header();
    build_libs();
    move_libs();
}

fn clean_dirs() {
    let header = PathBuf::from(format!("{INC}{HEAD}"));
    if header.exists() {
        fs::remove_file(header).unwrap();
    }

    let lib = PathBuf::from(format!("{IOS_LIB_DIR}{LIB}"));
    if lib.exists() {
        fs::remove_file(lib).unwrap();
    }

    let lib = PathBuf::from(format!("{MAC_LIB_DIR}{LIB}"));
    if lib.exists() {
        fs::remove_file(lib).unwrap();
    }

    let lib_folders = PathBuf::from(MAC_LIB_DIR);
    fs::create_dir_all(lib_folders).unwrap();

    let lib_folders = PathBuf::from(IOS_LIB_DIR);
    fs::create_dir_all(lib_folders).unwrap();
}

fn header() {
    let header = Command::new("cbindgen")
        .args(["../lb_external_interface/src/c_interface.rs", "-l", "c"])
        .current_dir("libs/lb/lb-rs")
        .success_output();

    let mut f = File::create(format!("{INC}{HEAD}")).unwrap();
    f.write_all(&header.stdout).unwrap();
}

fn build_libs() {
    // Build the iOS targets
    Command::new("cargo")
        .args(["build", "--release", "--target=aarch64-apple-ios"])
        .current_dir("libs/lb_external_interface")
        .assert_success();

    // Build the macOS targets
    Command::new("cargo")
        .args(["build", "--release", "--target=x86_64-apple-darwin"])
        .current_dir("libs/lb_external_interface")
        .assert_success();
    Command::new("cargo")
        .args(["build", "--release", "--target=aarch64-apple-darwin"])
        .current_dir("libs/lb_external_interface")
        .assert_success();

    // lipo macOS binaries together
    fs::create_dir_all("target/universal-macos").unwrap();
    Command::new("lipo")
        .args([
            "-create",
            "-output",
            "target/universal-macos/liblb_external_interface.a",
            "target/x86_64-apple-darwin/release/liblb_external_interface.a",
            "target/aarch64-apple-darwin/release/liblb_external_interface.a",
        ])
        .assert_success();
}

fn move_libs() {
    fs::copy(format!("target/aarch64-apple-ios/release/{LIB}"), format!("{IOS_LIB_DIR}{LIB}"))
        .unwrap();
    fs::copy(format!("target/universal-macos/{LIB}"), format!("{MAC_LIB_DIR}{LIB}")).unwrap();
}
