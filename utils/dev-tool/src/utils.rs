use std::env::{self, VarError};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const SERVER_PORT: u16 = 8501;

pub trait CommandRunner {
    fn assert_success(&mut self);
    fn assert_success_with_output(&mut self) -> Output;
}

impl CommandRunner for Command {
    fn assert_success(&mut self) {
        if !self.status().unwrap().success() {
            panic!()
        }
    }

    fn assert_success_with_output(&mut self) -> Output {
        let output = self.output().unwrap();

        if !output.status.success() {
            panic!()
        }

        output
    }
}

pub fn get_api_url() -> String {
    format!("http://localhost:{}", SERVER_PORT)
}

pub fn android_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/android")
}

pub fn jni_lib_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    android_dir(root).join("core/src/main/jniLibs")
}

pub fn swift_core_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/apple/SwiftLockbookCore")
}

pub fn swift_inc<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref()
        .join("clients/apple/CLockbookCore/Sources/CLockbookCore/include")
}

pub fn swift_lib<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref()
        .join("clients/apple/CLockbookCore/Sources/CLockbookCore/lib")
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

pub fn get_commit_hash() -> String {
    let commit_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .stdout(Stdio::piped())
        .output()
        .unwrap()
        .stdout;

    String::from_utf8_lossy(commit_hash.as_slice())
        .trim()
        .to_string()
}

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

pub fn dev_dir() -> PathBuf {
    let home_dir = env::var("HOME").unwrap();

    PathBuf::from(home_dir).join(".lockbook-dev")
}

pub fn target_dir<P: AsRef<Path>>(dev_dir: P, root_dir: P) -> PathBuf {
    if is_ci_env() {
        dev_dir.as_ref().join("target")
    } else {
        root_dir.as_ref().join("target")
    }
}

pub fn server_dbs_dir<P: AsRef<Path>>(dev_dir: P) -> PathBuf {
    dev_dir.as_ref().join("server-db")
}

pub fn is_ci_env() -> bool {
    match env::var("LOCKBOOK_CI") {
        Ok(is_ci) => match is_ci.as_str() {
            "1" => true,
            "0" => false,
            _ => panic!("Unknown ci state: {}", is_ci),
        },
        Err(e) => match e {
            VarError::NotPresent => false,
            _ => panic!("Unknown ci state: {:?}", e),
        },
    }
}
