use crate::utils::{self, CommandRunner, SERVER_PORT};
use crate::ToolEnvironment;

use std::fs;
use std::fs::File;
use std::process::{Command, Stdio};

pub fn run_server_detached(tool_env: &ToolEnvironment) {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    let server_log = File::create(utils::server_log(&tool_env.dev_dir)).unwrap();

    let out = Stdio::from(server_log);

    let mut run_result = Command::new("cargo")
        .args(["run", "-p", "lockbook-server", "--release"])
        .current_dir(&tool_env.root_dir)
        .stderr(Stdio::null())
        .stdout(out)
        .spawn()
        .unwrap();

    loop {
        if run_result.try_wait().unwrap().is_some() {
            panic!("Server failed to start.")
        }

        if reqwest::blocking::get("http://localhost:8000/get-build-info").is_ok() {
            break;
        }
    }
}

pub fn kill_server(tool_env: &ToolEnvironment) {
    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", SERVER_PORT)])
        .assert_success();

    fs::remove_dir_all("/tmp/lbdev").unwrap();
    fs::remove_file(utils::server_log(&tool_env.dev_dir)).unwrap();
}

pub fn print_server_logs(tool_env: &ToolEnvironment) {
    let logs = utils::server_log(&tool_env.dev_dir);

    println!("{}", fs::read_to_string(logs).unwrap())
}

pub fn run_rust_tests(tool_env: &ToolEnvironment) {
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
