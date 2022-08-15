use crate::utils::{self, CommandRunner, HashInfo};
use crate::ToolEnvironment;

use rand::Rng;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{fs, thread};

pub fn run_server_detached(tool_env: &ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();
    let mut port;
    let server_db_dir = tool_env.server_dbs_dir.join(&tool_env.hash_info_dir);

    Command::new("cargo")
        .args(["build", "-p", "lockbook-server", "--release"])
        .assert_success();

    loop {
        port = rand::thread_rng().gen_range(1024..u16::MAX);

        let mut run_result = Command::new(tool_env.target_dir.join("debug/lockbook-server"))
            .env("SERVER_PORT", port.to_string())
            .env("INDEX_DB_LOCATION", &server_db_dir)
            .current_dir(&tool_env.root_dir)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();

        thread::sleep(Duration::from_millis(5000));

        if run_result.try_wait().unwrap().is_none() {
            break;
        }
    }

    let hash_info =
        HashInfo::new(&tool_env.hash_info_dir, &server_db_dir, &tool_env.commit_hash, port);
    hash_info.save();
}

pub fn kill_server(tool_env: &ToolEnvironment) {
    let maybe_hash_info =
        HashInfo::maybe_get_from_dir(&tool_env.hash_info_dir, &tool_env.commit_hash);

    if let Some(hash_info) = maybe_hash_info {
        kill_server_at_port(&hash_info);
        hash_info.delete();
    }
}

fn kill_server_at_port(hash_info: &HashInfo) {
    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", hash_info.port)])
        .assert_success();
}

pub fn kill_all_servers(tool_env: &ToolEnvironment) {
    let children = fs::read_dir(&tool_env.hash_info_dir).unwrap();

    for child in children {
        let path = child.unwrap().path();
        let maybe_hash_info = HashInfo::maybe_get_at_path(&path);
        if let Some(hash_info) = maybe_hash_info {
            kill_server_at_port(&hash_info);
            hash_info.delete();
        }
    }
}

pub fn run_rust_tests(tool_env: &ToolEnvironment) {
    let hash_info = HashInfo::get_from_dir(&tool_env.hash_info_dir, &tool_env.commit_hash);
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["test", "--workspace", "--release"])
        .env("API_URL", utils::get_api_url(hash_info.port))
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
