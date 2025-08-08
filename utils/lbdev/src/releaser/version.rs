use cli_rs::cli_error::CliResult;
use regex::{Captures, Regex};
use std::fmt::{Display, Formatter};
use std::fs;
use std::process::Command;
use std::str::FromStr;
use time::OffsetDateTime;
use toml_edit::{Document, value};

use crate::utils::CommandRunner;

use super::utils::lb_version;

pub fn bump(bump_type: BumpType) -> CliResult<()> {
    let new_version = determine_new_version(bump_type);

    ensure_clean_start_state();

    handle_cargo_tomls(&new_version);
    handle_apple(&new_version)?;
    handle_android(&new_version);
    generate_lockfile()?;
    perform_checks()?;
    push_to_git(&new_version);

    Ok(())
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum BumpType {
    Major,
    Minor,

    #[default]
    Patch,
}

impl FromStr for BumpType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            _ => Err(format!("{s} is not patch minor or major")),
        }
    }
}

impl Display for BumpType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_ascii_lowercase())
    }
}

fn handle_cargo_tomls(version: &str) {
    // todo: I wonder if we should just read the workspace members...
    let cargos_to_update = vec![
        "libs/lb/lb-rs",
        "libs/lb/test_utils",
        "libs/lb/lb-c",
        "libs/lb/lb-java",
        "libs/content/workspace",
        "libs/content/workspace-ffi",
        "libs/lb-fs",
        "server",
        "clients/cli",
        "clients/egui",
        "clients/linux",
        "clients/windows",
        "clients/admin",
        "utils/dev-tool",
        "utils/releaser",
        "utils/winstaller",
    ];

    for cargo_path in cargos_to_update {
        let cargo_path = &[cargo_path, "/Cargo.toml"].join("");
        let mut cargo_toml = fs::read_to_string(cargo_path)
            .unwrap()
            .parse::<Document>()
            .unwrap();

        cargo_toml["package"]["version"] = value(version);
        fs::write(cargo_path, cargo_toml.to_string()).unwrap();
    }
}

fn handle_apple(version: &str) -> CliResult<()> {
    let plists = ["clients/apple/iOS/info.plist", "clients/apple/macOS/info.plist"];
    for plist in plists {
        Command::new("/usr/libexec/Plistbuddy")
            .args(["-c", &format!("Set CFBundleShortVersionString {version}"), plist])
            .assert_success()?;
        let now = OffsetDateTime::now_utc();

        let month = now.month() as u8;
        let day = now.day();
        let year = now.year();

        // add leading zeros where missing
        let month = format!("{month:0>2}");
        let day = format!("{day:0>2}");

        Command::new("/usr/libexec/Plistbuddy")
            .args(["-c", &format!("Set CFBundleVersion {year}{month}{day}"), plist])
            .assert_success()?;
    }

    Ok(())
}

fn handle_android(version: &str) {
    let path = "clients/android/app/build.gradle";
    let mut gradle_build = fs::read_to_string(path).unwrap();

    let version_name_re = Regex::new(r"(versionName) (.*)").unwrap();
    let version_code_re = Regex::new(r"(versionCode) *(?P<version_code>\d+)").unwrap();
    let mut version_code = 0;
    for caps in version_code_re.captures_iter(&gradle_build) {
        version_code = caps["version_code"].parse().unwrap();
    }
    gradle_build = version_code_re
        .replace(&gradle_build, |caps: &Captures| format!("{} {}", &caps[1], version_code + 1))
        .to_string();

    gradle_build = version_name_re
        .replace(&gradle_build, |caps: &Captures| format!("{} \"{}\"", &caps[1], version))
        .to_string();

    fs::write(path, gradle_build).unwrap();
}

fn determine_new_version(bump_type: BumpType) -> String {
    let mut current_version: Vec<i32> = lb_version()
        .split('.')
        .map(|f| f.parse().unwrap())
        .collect();

    match bump_type {
        BumpType::Major => {
            current_version[0] += 1;
            current_version[1] = 0;
            current_version[2] = 0;
        }
        BumpType::Minor => {
            current_version[1] += 1;
            current_version[2] = 0;
        }
        BumpType::Patch => current_version[2] += 1,
    }

    current_version
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<String>>()
        .join(".")
}

fn generate_lockfile() -> CliResult<()> {
    Command::new("cargo").arg("check").assert_success()
}

fn ensure_clean_start_state() {
    Command::new("git")
        .args(["diff", "--exit-code"])
        .assert_success()
        .unwrap()
}

fn push_to_git(version: &str) {
    Command::new("bash")
        .args([
            "-c",
            &format!("git add -A && git commit -m 'bump-{version}' && git push origin master"),
        ])
        .assert_success()
        .unwrap()
}

fn perform_checks() -> CliResult<()> {
    Command::new("bash")
        .args(["-c", "cargo check"])
        .assert_success()
}
