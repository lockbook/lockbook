use crate::error::CliError;
use execute_command_macro::command;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, fs};

pub fn android_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/android")
}

pub fn swift_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/apple/SwiftLockbookCore")
}

pub fn core_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("core")
}

pub fn local_env_path<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("containers/local.env")
}

pub fn test_env_path<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("containers/test.env")
}

pub fn get_root_env_dir() -> Result<PathBuf, CliError> {
    let proj_root = command!("git rev-parse --show-toplevel")
        .stdout(Stdio::piped())
        .output()?
        .stdout;

    Ok(PathBuf::from(
        String::from_utf8_lossy(proj_root.as_slice())
            .to_string()
            .trim(),
    ))
}

pub fn get_commit_hash() -> Result<String, CliError> {
    let commit_hash = command!("git rev-parse HEAD")
        .stdout(Stdio::piped())
        .output()?
        .stdout;

    Ok(String::from_utf8_lossy(commit_hash.as_slice())
        .trim()
        .to_string())
}

pub fn get_target_dir() -> Result<String, CliError> {
    Ok(format!("{}/.cargo/lockbook-dev-target", env::var("HOME")?))
}

pub fn get_hash_port_dir(commit_hash: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/lbdev/{}.ec", commit_hash))
}

#[derive(Serialize, Deserialize)]
pub struct HashInfo {
    pub maybe_port: Option<u16>,
    pub server_binary_path: PathBuf,
}

impl HashInfo {
    pub fn get_port(&self) -> Result<u16, CliError> {
        Ok(self
            .maybe_port
            .ok_or(CliError(Some("Server not running.".to_string())))?)
    }

    pub fn get_from_disk(commit_hash: &str) -> Result<HashInfo, CliError> {
        let port_dir = get_hash_port_dir(commit_hash);
        let contents = fs::read_to_string(&port_dir)?;

        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self, commit_hash: &str) -> Result<(), CliError> {
        File::create(get_hash_port_dir(commit_hash))?
            .write_all(serde_json::to_string(self)?.as_bytes())?;

        Ok(())
    }
}

pub fn get_api_url(port: u16) -> String {
    format!("http://lockbook_server:{}", port)
}
