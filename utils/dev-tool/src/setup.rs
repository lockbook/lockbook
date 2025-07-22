use std::env;
use std::process::{Command, Stdio};

const REQUIRED_COMMANDS: [&str; 3] = ["cargo", "cbindgen", "swift"];
const REQUIRED_ENV_VARS: [&str; 1] = ["ANDROID_HOME"];

pub fn verify_ci_environment() {
    for command in REQUIRED_COMMANDS {
        let command_result = Command::new("which")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();

        if !command_result.success() {
            panic!("You do not have '{command}'");
        }
    }

    for env_var in REQUIRED_ENV_VARS {
        if env::var(env_var).is_err() {
            panic!("'{env_var}' environment variable is not set.");
        }
    }
}
