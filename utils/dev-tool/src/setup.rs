use std::env;
use std::process::{Command, Stdio};

const NEEDED_COMMANDS: [&str; 4] = ["cargo", "cbindgen", "swift", "cargo-ndk"];
const NEEDED_ENV_VARS: [&str; 2] = ["ANDROID_HOME", "ANDROID_NDK"];

pub fn verify_ci_environment() {
    for command in NEEDED_COMMANDS {
        verify_command_exists(command);
    }

    for env_var in NEEDED_ENV_VARS {
        verify_env_var_exists(env_var);
    }
}

fn verify_command_exists(command: &str) {
    let command_result = Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    if !command_result.success() {
        panic!("You do not have '{}'", command);
    }
}

fn verify_env_var_exists(env_var: &str) {
    if env::var(env_var).is_err() {
        panic!("'{}' environment variable is not set.", env_var);
    }
}
