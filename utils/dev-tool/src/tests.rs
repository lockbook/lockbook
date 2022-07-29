use std::env;
use std::process::Command;
use crate::{CliError, utils};

pub fn launch_server_detached() -> Result<(), CliError> {
    let port = port_scanner::request_open_port().ok_or(CliError(Some("Cannot find an open local port.".to_string())))?;

    dotenv::from_path(utils::local_env_path(env::current_dir()?))?;

    env::set_var("SERVER_PORT", port);

    let mut command = Command::new("./gradlew");

    utils::in_android_dir(&mut command)?;

    let fmt_result = command
        .arg("lintKotlin")
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error())
    }

    Ok(())
}

