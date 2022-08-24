use crate::utils::{self, CommandRunner, SERVER_PORT};
use crate::ToolEnvironment;

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn run_server_detached(tool_env: &ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();
    let server_db_dir = tool_env.server_dbs_dir.join(&tool_env.commit_hash);

    Command::new("cargo")
        .args(["build", "-p", "lockbook-server", "--release"])
        .assert_success();

    let mut run_result = Command::new(tool_env.target_dir.join("release/lockbook-server"))
        .env("SERVER_PORT", SERVER_PORT.to_string())
        .env("INDEX_DB_LOCATION", &server_db_dir)
        .current_dir(&tool_env.root_dir)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(5000));

    if run_result.try_wait().unwrap().is_none() {
        panic!("Server failed to start on port: '{}'", SERVER_PORT)
    }
}

pub fn kill_server() {
    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", SERVER_PORT)])
        .assert_success();
}

pub fn run_rust_tests(tool_env: &ToolEnvironment) {
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["test", "--workspace"])
        .env("API_URL", utils::get_api_url())
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
