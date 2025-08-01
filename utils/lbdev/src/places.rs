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
    let mut path = root_dir();
    path.push("server");
    path.push("local.env");
    path
}

pub fn server_log() -> PathBuf {
    let mut path = root_dir();
    path.push("server");
    path.push("server_log.txt");
    path
}

pub fn android_dir() -> PathBuf {
    let mut path = root_dir();
    path.push("clients");
    path.push("android");
    path
}

pub fn workspace_ffi() -> PathBuf {
    let mut path = root_dir();

    path.push("libs");
    path.push("content");
    path.push("workspace-ffi");

    path
}
