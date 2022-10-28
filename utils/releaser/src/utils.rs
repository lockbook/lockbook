use gh_release::RepoInfo;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::{env, fs};
use toml::Value;

pub trait CommandRunner {
    fn assert_success(&mut self);
    fn success_output(&mut self) -> Output;
}

impl CommandRunner for Command {
    fn assert_success(&mut self) {
        if !self.status().unwrap().success() {
            panic!()
        }
    }

    fn success_output(&mut self) -> Output {
        let out = self.output().unwrap();

        if !out.status.success() {
            panic!("{:#?}", out)
        }

        out
    }
}

pub fn lb_repo() -> RepoInfo<'static> {
    RepoInfo { owner: "lockbook", repo_name: "lockbook" }
}

pub fn core_version() -> String {
    let core = fs::read_to_string("core/Cargo.toml").unwrap();
    core.parse::<Value>().unwrap()["package"]["version"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn android_version_code() -> String {
    let version_bytes = Command::new("./gradlew")
        .args(&["-q", "printVersionCode"])
        .current_dir("clients/android")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .success_output()
        .stdout;

    String::from_utf8_lossy(version_bytes.as_slice())
        .trim()
        .to_string()
}

pub fn sha_file(file: &str) -> String {
    let bytes = fs::read(file).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

pub fn root() -> PathBuf {
    let project_root = env::current_dir().unwrap();
    if project_root.file_name().unwrap() != "lockbook" {
        panic!("releaser not called from project root");
    }
    project_root
}
