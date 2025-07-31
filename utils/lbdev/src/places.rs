use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn root_dir() -> PathBuf {
    let root_bytes = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stdout(Stdio::piped())
        .output()
        .unwrap()
        .stdout;

    PathBuf::from(
        String::from_utf8_lossy(root_bytes.as_slice())
            .trim()
            .to_string(),
    )
}

pub fn local_env_path() -> PathBuf {
    root_dir().join("server/local.env")
}

pub fn server_log() -> PathBuf {
    root_dir().join("server/server_log.txt")
}

pub fn android_dir() -> PathBuf {
    root_dir().join("clients/android")
}
