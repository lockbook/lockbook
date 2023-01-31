use gh_release::RepoInfo;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::{env, fs};
use toml::Value;
use toml_edit::{value, Document};

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
    let core = fs::read_to_string("libs/core/Cargo.toml").unwrap();
    core.parse::<Value>().unwrap()["package"]["version"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn android_version_code() -> String {
    let version_bytes = Command::new("./gradlew")
        .args(["-q", "printVersionCode"])
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

pub fn determine_new_version(bump_type: Option<String>) -> Option<String> {
    let mut current_version: Vec<i32> = core_version()
        .split('.')
        .map(|f| f.parse().unwrap())
        .collect();

    let bump_type = bump_type.unwrap_or("patch".to_string());
    match bump_type.as_ref() {
        "major" => current_version[0] += 1,
        "minor" => current_version[1] += 1,
        "patch" => current_version[2] += 1,
        _ => panic!(
            "{} is an undefined version bump. accepted values are: major, minor, patch",
            bump_type
        ),
    }

    let new_version = current_version
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<String>>()
        .join(".");

    Some(new_version)
}

pub fn edit_cargo_version(cargo_path: &str, version: &str) {
    let mut server = fs::read_to_string(cargo_path)
        .unwrap()
        .parse::<Document>()
        .unwrap();

    server["package"]["version"] = value(version);

    fs::write(cargo_path, server.to_string()).unwrap();
}
