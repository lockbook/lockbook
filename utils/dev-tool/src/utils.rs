use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::fs::{self, File};
use std::io::Write;
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

pub fn get_api_url(port: u16) -> String {
    format!("http://localhost:{}", port)
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

pub fn swift_core_dir<P: AsRef<Path>>(root: P) -> PathBuf {
    swift_dir(root).join("SwiftLockbookCore")
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

pub fn hash_infos_dir<P: AsRef<Path>>(dev_dir: P) -> PathBuf {
    dev_dir.as_ref().join("hash-info")
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

pub fn hash_info_dir(dev_dir: PathBuf, commit_hash: &str) -> PathBuf {
    dev_dir
        .join("hash-info")
        .join(format!("{}.json", commit_hash))
}

#[derive(Serialize, Deserialize)]
pub struct HashInfo {
    pub maybe_port: Option<u16>,
    pub hash_info_dir: PathBuf,
}

impl HashInfo {
    pub fn new<P: AsRef<Path>>(hash_infos_dir: P, commit_hash: &str) -> Self {
        let hash_info_dir = hash_infos_dir.as_ref().join(commit_hash);

        Self { maybe_port: None, hash_info_dir }
    }

    pub fn get_port(&self) -> u16 {
        self.maybe_port.unwrap()
    }

    pub fn get_from_dir<P: AsRef<Path>>(hash_infos_dir: P, commit_hash: &str) -> Self {
        let hash_info_dir = hash_infos_dir.as_ref().join(commit_hash);

        Self::maybe_get_at_path(hash_info_dir)
            .expect("No hash info file found. Server may not be running or even built!")
    }

    pub fn maybe_get_from_dir<P: AsRef<Path>>(
        hash_infos_dir: P, commit_hash: &str,
    ) -> Option<Self> {
        let hash_info_dir = hash_infos_dir.as_ref().join(commit_hash);
        Self::maybe_get_at_path(hash_info_dir)
    }

    pub fn maybe_get_at_path<P: AsRef<Path>>(hash_info_dir: P) -> Option<Self> {
        fs::read_to_string(hash_info_dir)
            .ok()
            .map(|contents| serde_json::from_str(&contents).unwrap())
    }

    pub fn save(&self) {
        File::create(&self.hash_info_dir)
            .unwrap()
            .write_all(serde_json::to_string(self).unwrap().as_bytes())
            .unwrap();
    }

    pub fn delete(&self) {
        fs::remove_file(&self.hash_info_dir).unwrap();
    }
}
