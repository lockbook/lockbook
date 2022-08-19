use github_release_rs::RepoInfo;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::process::Command;
use toml::Value;

pub trait CommandRunner {
    fn assert_success(&mut self);
}

impl CommandRunner for Command {
    fn assert_success(&mut self) {
        if !self.status().unwrap().success() {
            panic!()
        }
    }
}

pub fn lb_repo() -> RepoInfo {
    RepoInfo { owner: "lockbook".to_string(), repo_name: "lockbook".to_string() }
}

pub fn core_version() -> String {
    let core = fs::read_to_string("core/Cargo.toml").unwrap();
    core.parse::<Value>().unwrap()["package"]["version"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn sha_file(file: &str) -> String {
    let bytes = fs::read(file).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    hex::encode(result)
}
