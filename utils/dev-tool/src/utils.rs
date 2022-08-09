use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{env, fs};

pub fn panic_if_unsuccessful(e: ExitStatus) {
    if !e.success() {
        panic!()
    }
}

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

pub fn get_dirs() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let home_dir = env::var("HOME").unwrap();

    let root_dir = {
        let root_bytes = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .stdout(Stdio::piped())
            .output()
            .unwrap()
            .stdout;

        String::from_utf8_lossy(root_bytes.as_slice())
            .trim()
            .to_string()
    };

    let dev_dir = format!("{}/.lockbook-dev", home_dir);

    fs::create_dir_all(&dev_dir).unwrap();

    let target_dir =
        if is_ci_env() { format!("{}/target", dev_dir) } else { format!("{}/target", root_dir) };

    let sdk_dir = format!("{}/{}", dev_dir, "android-sdk");

    fs::create_dir_all(&sdk_dir).unwrap();

    (
        PathBuf::from(home_dir),
        PathBuf::from(root_dir),
        PathBuf::from(target_dir),
        PathBuf::from(sdk_dir),
    )
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

pub fn get_hash_port_dir(commit_hash: &str) -> PathBuf {
    tmp_dir().join(format!("{}.ec", commit_hash))
}

#[derive(Serialize, Deserialize)]
pub struct HashInfo {
    pub maybe_port: Option<u16>,
    pub server_binary_path: PathBuf,
}

impl HashInfo {
    pub fn get_port(&self) -> u16 {
        self.maybe_port.unwrap()
    }

    pub fn get_from_disk(commit_hash: &str) -> HashInfo {
        let port_dir = get_hash_port_dir(commit_hash);
        let contents = fs::read_to_string(&port_dir).unwrap();

        serde_json::from_str(&contents).unwrap()
    }

    pub fn save(&self, commit_hash: &str) {
        File::create(get_hash_port_dir(commit_hash))
            .unwrap()
            .write_all(serde_json::to_string(self).unwrap().as_bytes())
            .unwrap();
    }
}

pub fn get_api_url(port: u16) -> String {
    format!("http://localhost:{}", port)
}
