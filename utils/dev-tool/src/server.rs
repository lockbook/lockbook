use crate::utils::{CommandRunner, HashInfo};
use crate::{utils, ToolEnvironment};

use rand::Rng;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{fs, thread};

pub fn build_server(tool_env: ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["build", "-p", "lockbook-server"])
        .assert_success();

    let server_path = tool_env.target_dir.join("debug/lockbook-server");
    let new_server_path =
        server_path.with_file_name(format!("lockbook-server-{}", tool_env.commit_hash));

    fs::rename(server_path, &new_server_path).unwrap();

    let hash_info = HashInfo::new(&tool_env.hash_info_dir, &new_server_path, &tool_env.commit_hash);
    hash_info.save();
}

pub fn run_server_detached(tool_env: ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();
    let mut hash_info = HashInfo::get_from_dir(&tool_env.hash_info_dir, &tool_env.commit_hash);

    let mut port;

    loop {
        println!("STARTED");
        port = rand::thread_rng().gen_range(1024..u16::MAX);

        let mut run_result = Command::new(&hash_info.server_binary_path)
            .env("SERVER_PORT", port.to_string())
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();

        thread::sleep(Duration::from_millis(5000));

        if run_result.try_wait().unwrap().is_none() {
            println!("RERUNNING");
            break;
        }
    }

    hash_info.maybe_port = Some(port);
    hash_info.save();
}

pub fn kill_server(tool_env: ToolEnvironment) {
    let mut hash_info = HashInfo::get_from_dir(&tool_env.hash_info_dir, &tool_env.commit_hash);

    kill_server_at_port(&hash_info);

    hash_info.maybe_port = None;
    hash_info.save();
}

fn kill_server_at_port(hash_info: &HashInfo) {
    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", hash_info.get_port())])
        .assert_success();
}

pub fn kill_all_servers(tool_env: ToolEnvironment) {
    let children = fs::read_dir(&tool_env.hash_info_dir).unwrap();

    for child in children {
        let path = child.unwrap().path();
        kill_server_at_port(&HashInfo::get_at_path(&path));

        fs::remove_file(path).unwrap();
    }
}

pub fn run_rust_tests(tool_env: ToolEnvironment) {
    let hash_info = HashInfo::get_from_dir(&tool_env.hash_info_dir, &tool_env.commit_hash);
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["test", "--release", "--no-fail-fast", "--all", "--", "--nocapture"])
        .env("API_URL", utils::get_api_url(hash_info.get_port()))
        .current_dir(tool_env.root_dir)
        .assert_success();
}
