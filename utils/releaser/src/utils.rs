use gh_release::RepoInfo;
use regex::{Captures, Regex};
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

    let bump_type = bump_type.unwrap_or_else(|| "patch".to_string());
    match bump_type.as_ref() {
        "major" => {
            current_version[0] += 1;
            current_version[1] = 0;
            current_version[2] = 0;
        }
        "minor" => {
            current_version[1] += 1;
            current_version[2] = 0;
        }
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
    let mut server = fs::read_to_string([cargo_path, "Cargo.toml"].join(""))
        .unwrap()
        .parse::<Document>()
        .unwrap();

    server["package"]["version"] = value(version);

    fs::write(cargo_path, server.to_string()).unwrap();
}
//TODO: test this out and make sure the regex works
pub fn edit_android_version(version: &str) {
    let path = "clients/android/build.gradle";
    let mut gradle_build = fs::read_to_string(path).unwrap();

    let version_code_re = Regex::new(r"versionCode *(?P<version_code>\d+)").unwrap();
    let mut version_code = 0;
    for caps in version_code_re.captures_iter(&gradle_build) {
        version_code = caps["version_code"].parse().unwrap();
    }
    gradle_build = gradle_build.replace(&version_code.to_string(), &(version_code + 1).to_string());

    let version_name_re = Regex::new(r"(versionName) (.*)").unwrap();
    gradle_build = version_name_re
        .replace(&gradle_build, |caps: &Captures| format!("{}{}", &caps[1], version))
        .to_string();

    fs::write(path, gradle_build).unwrap();
}

pub fn bump_versions(bump_type: Option<String>) {
    let new_version = determine_new_version(bump_type).unwrap_or_else(core_version);
    let new_version = new_version.as_str();

    let cargos_to_update = vec![
        "clients/admin/",
        "clients/cli/",
        "clients/egui/",
        "server/server/",
        "libs/core/",
        "libs/core/libs/shared/",
        "libs/core/libs/test_utils/",
        "libs/editor/equi_editor",
        "utils/dev-tool",
        "utils/releaser",
        "utils/winstaller",
    ];
    for cargo_path in cargos_to_update {
        edit_cargo_version(cargo_path, new_version);
    }

    //apple
    //TODO figure out what sbuild number and appropriate plist
    Command::new("/usr/libexec/Plistbuddy").args([
        "-c",
        "Set CFBundleVersion $SBUILD_NUMBER",
        "$PLIST",
    ]);

    Command::new("/usr/libexec/Plistbuddy").args([
        "-c",
        "Set CFBundleShortVersionString $BUNDLE_SHORT_VERSION",
        "$PLIST",
    ]);

    edit_android_version(new_version)
}
