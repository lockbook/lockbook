use crate::utils::HashInfo;
use crate::{utils, ToolEnvironment};

use std::fs;
use std::process::{Command, Stdio};

pub fn build_server(tool_env: ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    let build_results = Command::new("cargo")
        .args(["build", "-p", "lockbook-server"])
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(build_results);

    let server_path = tool_env.target_dir.join("debug/lockbook-server");
    let new_server_path =
        server_path.with_file_name(format!("lockbook-server-{}", tool_env.commit_hash));

    fs::rename(server_path, &new_server_path).unwrap();

    let hash_info = HashInfo { maybe_port: None, server_binary_path: new_server_path };
    hash_info.save(&tool_env.commit_hash);
}

pub fn run_server_detached(tool_env: ToolEnvironment) {
    let port = port_scanner::request_open_port().unwrap();

    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    let mut hash_info = HashInfo::get_from_disk(&tool_env.commit_hash);

    Command::new(&hash_info.server_binary_path)
        .env("SERVER_PORT", port.to_string())
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    hash_info.maybe_port = Some(port);
    hash_info.save(&tool_env.commit_hash);
}

pub fn kill_server(tool_env: ToolEnvironment) {
    let mut hash_info = HashInfo::get_from_disk(&tool_env.commit_hash);

    let kill_result = Command::new("fuser")
        .args(["-k", &format!("{}/tcp", hash_info.get_port().to_string())])
        .current_dir(utils::swift_dir(&tool_env.root_dir))
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(kill_result);

    hash_info.maybe_port = None;
    hash_info.save(&tool_env.commit_hash);
}

pub fn run_rust_tests(tool_env: ToolEnvironment) {
    let hash_info = HashInfo::get_from_disk(&tool_env.commit_hash);
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir)).unwrap();

    let test_results = Command::new("cargo")
        .args(["test", "--release", "--no-fail-fast", "--all", "--", "--nocapture"])
        .env("API_URL", utils::get_api_url(hash_info.get_port()))
        .current_dir(tool_env.root_dir)
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(test_results);
}
