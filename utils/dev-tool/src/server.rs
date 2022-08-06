use crate::utils::HashInfo;
use crate::{utils, CliError, ToolEnvironment};
use execute_command_macro::{command, command_args};
use std::fs;
use std::process::{Command, Stdio};

pub fn build_server(tool_env: ToolEnvironment) -> Result<(), CliError> {
    dotenv::from_path(utils::local_env_path(&tool_env.root_dir))?;

    let build_results = command!("cargo build -p lockbook-server").spawn()?.wait()?;

    if !build_results.success() {
        return Err(CliError::basic_error());
    }

    let server_path = tool_env.target_dir.join("debug/lockbook-server");
    let new_server_path =
        server_path.with_file_name(format!("lockbook-server-{}", tool_env.commit_hash));

    fs::rename(server_path, &new_server_path)?;

    let hash_info = HashInfo { maybe_port: None, server_binary_path: new_server_path };
    hash_info.save(&tool_env.commit_hash)?;

    Ok(())
}

pub fn run_server_detached(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let port = port_scanner::request_open_port()
        .ok_or_else(|| CliError(Some("Cannot find an open local port.".to_string())))?;

    dotenv::from_path(utils::local_env_path(&tool_env.root_dir))?;

    let mut hash_info = HashInfo::get_from_disk(&tool_env.commit_hash)?;
    let server_path = hash_info
        .server_binary_path
        .to_str()
        .ok_or(CliError(Some("Cannot get server binary path!".to_string())))?;

    command_args!(server_path)
        .env("SERVER_PORT", port.to_string())
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()?;

    hash_info.maybe_port = Some(port);
    hash_info.save(&tool_env.commit_hash)?;

    Ok(())
}

pub fn kill_server(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let mut hash_info = HashInfo::get_from_disk(&tool_env.commit_hash)?;

    let kill_result =
        command_args!("fuser", "-k", format!("{}/tcp", hash_info.get_port()?.to_string()))
            .current_dir(utils::swift_dir(&tool_env.root_dir))
            .spawn()?
            .wait()?;

    if !kill_result.success() {
        return Err(CliError::basic_error());
    }

    hash_info.maybe_port = None;
    hash_info.save(&tool_env.commit_hash)?;

    Ok(())
}

pub fn run_rust_tests(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let hash_info = HashInfo::get_from_disk(&tool_env.commit_hash)?;
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir))?;

    let test_results = command!("cargo test --release --no-fail-fast --all -- --nocapture")
        .env("API_URL", utils::get_api_url(hash_info.get_port()?))
        .current_dir(tool_env.root_dir)
        .spawn()?
        .wait()?;

    if !test_results.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
