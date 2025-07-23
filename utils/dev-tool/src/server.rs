use crate::ToolEnvironment;
use crate::utils::{self, CommandRunner};

use std::fs::File;
use std::process::{Command, Stdio};
use std::{env, fs};

pub fn run_server_detached(tool_env: &ToolEnvironment) {
    dotenvy::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    let server_log = File::create(utils::server_log(&tool_env.root_dir)).unwrap();
    let out = Stdio::from(server_log);
    let port = env::var("SERVER_PORT").unwrap();
    let build_info_address = &utils::build_info_address(&port);

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

        if reqwest::blocking::get(build_info_address).is_ok() {
            println!("Server running on '{}'", utils::api_url(&port));
            break;
        }
    }
}

pub fn kill_server(tool_env: &ToolEnvironment) {
    dotenvy::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    Command::new("fuser")
        .args(["-k", &format!("{}/tcp", env::var("SERVER_PORT").unwrap())])
        .assert_success();

    fs::remove_dir_all("/tmp/lbdev").unwrap();
    fs::remove_file(utils::server_log(&tool_env.root_dir)).unwrap();
}

pub fn print_server_logs(tool_env: &ToolEnvironment) {
    let logs = utils::server_log(&tool_env.root_dir);

    println!("{}", fs::read_to_string(logs).unwrap())
}

pub fn run_rust_tests(tool_env: &ToolEnvironment) {
    dotenvy::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
