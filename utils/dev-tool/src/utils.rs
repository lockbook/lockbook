use crate::error::CliError;
use execute_command_macro::command;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, fs};

pub fn tmp_dir() -> PathBuf {
    PathBuf::from("/tmp")
}

pub fn android_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/android")
}

pub fn jni_lib_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    android_dir(root).join("core/src/main/jniLibs")
}

pub fn swift_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/apple/SwiftLockbookCore")
}

pub fn swift_inc<P: AsRef<Path>>(root: P) -> PathBuf {
    swift_dir(root).join("Sources/CLockbookCore/include")
}

pub fn swift_lib<P: AsRef<Path>>(root: P) -> PathBuf {
    swift_dir(root).join("Sources/CLockbookCore/lib")
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

pub fn get_commit_hash() -> Result<String, CliError> {
    let commit_hash = command!("git rev-parse HEAD")
        .stdout(Stdio::piped())
        .output()?
        .stdout;

    Ok(String::from_utf8_lossy(commit_hash.as_slice())
        .trim()
        .to_string())
}

pub fn get_root_and_target_dir() -> Result<(PathBuf, PathBuf), CliError> {
    let root_dir = {
        let root_bytes = command!("git rev-parse --show-toplevel")
            .stdout(Stdio::piped())
            .output()?
            .stdout;

        String::from_utf8_lossy(root_bytes.as_slice())
            .trim()
            .to_string()
    };

    let target_dir = if is_ci_env()? {
        format!("{}/.lockbook-dev/rust-target", env::var("HOME")?)
    } else {
        format!("{}/target", root_dir)
    };

    Ok((PathBuf::from(root_dir), PathBuf::from(target_dir)))
}

pub fn is_ci_env() -> Result<bool, CliError> {
    match env::var("LOCKBOOK_CI") {
        Ok(is_ci) => match is_ci.as_str() {
            "1" => Ok(true),
            "0" => Ok(false),
            _ => Err(CliError(Some(format!("Unknown ci state: {}", is_ci)))),
        },
        Err(e) => match e {
            VarError::NotPresent => Ok(false),
            _ => Err(CliError::from(e)),
        },
    }
}

pub fn get_hash_port_dir(commit_hash: &str) -> PathBuf {
    tmp_dir().join(format!("{}.ec", commit_hash))
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
    format!("http://localhost:{}", port)
}
