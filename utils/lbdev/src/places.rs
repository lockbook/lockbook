use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn root() -> PathBuf {
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
    let mut path = root();
    path.push("server");
    path.push("local.env");
    path
}

pub fn server_log() -> PathBuf {
    let mut path = root();
    path.push("server");
    path.push("server_log.txt");
    path
}

pub fn android_dir() -> PathBuf {
    let mut path = root();
    path.push("clients");
    path.push("android");
    path
}

pub fn workspace_ffi() -> PathBuf {
    let mut path = root();

    path.push("libs");
    path.push("content");
    path.push("workspace-ffi");

    path
}

pub fn workspace_swift_libs() -> PathBuf {
    let mut path = workspace_ffi();

    path.push("SwiftWorkspace");
    path.push("Libs");

    path
}

pub fn target() -> PathBuf {
    let mut path = root();

    path.push("target");

    path
}
