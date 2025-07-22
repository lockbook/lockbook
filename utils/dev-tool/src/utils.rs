use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

pub fn android_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("clients/android")
}

pub fn jni_lib_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    android_dir(root).join("lb-rs/src/main/jniLibs")
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

pub fn lb_external_interface_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("libs/lb/lb_external_interface")
}

pub fn local_env_path<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join("containers/local.env")
}

pub fn server_log<P: AsRef<Path>>(root_dir: P) -> PathBuf {
    root_dir.as_ref().join("server/server_log.txt")
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

pub fn target_dir<P: AsRef<Path>>(root_dir: P) -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root_dir.as_ref().join("target"))
}

pub fn api_url(port: &str) -> String {
    format!("http://localhost:{port}")
}

pub fn build_info_address(port: &str) -> String {
    format!("{}/get-build-info", api_url(port))
}
