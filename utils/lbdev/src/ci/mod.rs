use std::{
    env,
    fs::{self, File},
    process::{Command, Stdio},
};

use cli_rs::cli_error::CliResult;

use crate::{
    places::{android_dir, local_env_path, root, server_log},
    utils::CommandRunner,
};

pub fn fmt() -> CliResult<()> {
    Command::new("cargo")
        .args(["fmt", "--", "--check", "-l"])
        .current_dir(root())
        .assert_success()
}

pub fn clippy() -> CliResult<()> {
    Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(root())
        .assert_success()
}

pub fn run_server_detached() -> CliResult<()> {
    dotenvy::from_path(local_env_path()).unwrap();

    let server_log = File::create(server_log()).unwrap();
    let out = Stdio::from(server_log);
    let port = env::var("SERVER_PORT").unwrap();
    let build_info_address = &build_info_address(&port);

    let mut run_result = Command::new("cargo")
        .args(["run", "-p", "lockbook-server", "--release"])
        .current_dir(root())
        .stderr(Stdio::null())
        .stdout(out)
        .spawn()
        .unwrap();

    loop {
        if run_result.try_wait().unwrap().is_some() {
            panic!("Server failed to start.")
        }

        if reqwest::blocking::get(build_info_address).is_ok() {
            println!("Server running on '{}'", api_url(&port));
            break;
        }
    }

    Ok(())
}

pub fn run_rust_tests() -> CliResult<()> {
    dotenvy::from_path(local_env_path()).unwrap();

    Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(root())
        .assert_success()
}

pub fn kill_server() -> CliResult<()> {
    dotenvy::from_path(local_env_path()).unwrap();

    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", env::var("SERVER_PORT").unwrap())])
        .assert_success()?;

    fs::remove_dir_all("/tmp/lbdev").unwrap();
    fs::remove_file(server_log()).unwrap();

    Ok(())
}

pub fn print_server_logs() -> CliResult<()> {
    let logs = server_log();

    println!("{}", fs::read_to_string(logs).unwrap());
    Ok(())
}

pub fn lint_android() -> CliResult<()> {
    let android_dir = android_dir();

    // Kotlin code style, formatting, and simple errors4j.
    Command::new(android_dir.join("gradlew"))
        .arg("lintKotlin")
        .current_dir(&android_dir)
        .assert_success()?;

    // Android-specific issues, resource problems, API usage, security, performance, etc.
    Command::new(android_dir.join("gradlew"))
        .arg("lint")
        .current_dir(android_dir)
        .assert_success()
}

pub fn assert_git_clean() -> CliResult<()> {
    Command::new("git")
        .args(["diff", "--exit-code"])
        .current_dir(root())
        .assert_success()
}

fn api_url(port: &str) -> String {
    format!("http://localhost:{port}")
}

fn build_info_address(port: &str) -> String {
    format!("{}/get-build-info", api_url(port))
}

pub fn assert_no_udeps() -> CliResult<()> {
    Command::new("cargo")
        .args(["+nightly", "udeps"])
        .current_dir(root())
        .assert_success()
}
