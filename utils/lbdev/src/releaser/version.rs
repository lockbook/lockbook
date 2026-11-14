use chrono::Local;
use cli_rs::cli_error::CliResult;
use regex::{Captures, Regex};
use std::fmt::{Display, Formatter};
use std::fs;
use std::process::Command;
use std::str::FromStr;
use toml_edit::{Document, value};

use crate::utils::CommandRunner;

use super::utils::lb_version;

pub fn bump(bump_type: BumpType) -> CliResult<()> {
    let new_version = determine_new_version(bump_type);

    ensure_clean_start_state();

    handle_cargo_tomls(&new_version);
    handle_apple(&new_version)?;
    bump_android(&new_version);
    bump_metainfo(&new_version);
    generate_lockfile()?;
    perform_checks()?;
    push_to_git(&new_version);

    Ok(())
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum BumpType {
    Major,
    Minor,
    Patch,

    #[default]
    Today,
}

impl FromStr for BumpType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "today" => Ok(Self::Today),
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
        "clients/desktop",
        "clients/linux",
        "clients/windows",
        "clients/admin",
        "utils/lbdev",
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
    let path = "clients/apple/lockbook.xcodeproj/project.pbxproj";
    let mut pbxproj = fs::read_to_string(path).unwrap();

    let build = super::apple::build_number();

    let marketing_re = Regex::new(r"MARKETING_VERSION = [^;]+;").unwrap();
    pbxproj = marketing_re
        .replace_all(&pbxproj, format!("MARKETING_VERSION = {version};"))
        .to_string();

    let build_re = Regex::new(r"CURRENT_PROJECT_VERSION = [^;]+;").unwrap();
    pbxproj = build_re
        .replace_all(&pbxproj, format!("CURRENT_PROJECT_VERSION = {build};"))
        .to_string();

    fs::write(path, pbxproj).unwrap();

    Ok(())
}

pub fn bump_android(version: &str) {
    let path = "clients/android/app/build.gradle";
    let mut gradle_build = fs::read_to_string(path).unwrap();

    let version_code_re = Regex::new(r"(versionCode) *(?P<version_code>\d+)").unwrap();
    let mut version_code = 0;
    for caps in version_code_re.captures_iter(&gradle_build) {
        version_code = caps["version_code"].parse().unwrap();
    }
    gradle_build = version_code_re
        .replace(&gradle_build, |caps: &Captures| format!("{} {}", &caps[1], version_code + 1))
        .to_string();

    let version_name_re = Regex::new(r"(versionName) (.*)").unwrap();
    gradle_build = version_name_re
        .replace(&gradle_build, |caps: &Captures| format!("{} \"{}\"", &caps[1], version))
        .to_string();

    fs::write(path, gradle_build).unwrap();
}

fn bump_metainfo(version: &str) {
    let path = "utils/dev/flatpak-package/net.lockbook.Lockbook.metainfo.xml";
    let today = Local::now().format("%Y-%m-%d");

    let metainfo = METAINFO_TEMPLATE
        .replace("{version}", version)
        .replace("{today}", &today.to_string());

    fs::write(path, metainfo).unwrap();
}

pub fn set_android_version_code(code: i64) {
    let path = "clients/android/app/build.gradle";
    let mut gradle_build = fs::read_to_string(path).unwrap();

    let version_code_re = Regex::new(r"(versionCode) *(?P<version_code>\d+)").unwrap();

    gradle_build = version_code_re
        .replace(&gradle_build, |caps: &Captures| format!("{} {}", &caps[1], code))
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
        BumpType::Today => return Local::now().format("%y.%-m.%-d").to_string(),
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

const METAINFO_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>net.lockbook.Lockbook</id>

  <name>Lockbook</name>
  <summary>Write, sketch, and share privately</summary>
  <developer id="net.lockbook">
    <name>Lockbook</name>
  </developer>

  <metadata_license>MIT</metadata_license>
  <project_license>Unlicense</project_license>

  <description>
    <p>Lockbook is an open-source notebook for Markdown notes and drawings, encrypted on your device and shared only by invitation. Notes are Markdown files and drawings are SVGs, kept in ordinary folders. Open them in any other app and you'll find nothing missing.</p>
    <p>Your notes are encrypted on your device, so no one else can read them, not even us. Sign up with just a username: we never ask for an email, a phone number, or a name.</p>
    <ul>
      <li>Type punctuation and watch plain text turn into headings, lists, tables, and code</li>
      <li>Sketch, handwrite, and diagram on an infinite canvas</li>
      <li>Mention one note in another and see which notes mention it back</li>
      <li>Find a note by its name or by what's in it, privately and offline</li>
      <li>Keep photos, PDFs, and everything else worth keeping</li>
      <li>Share any note or folder with read or write access</li>
      <li>Edits sync without being dropped, duplicated, or clobbered</li>
      <li>Also on Windows, Mac, iPhone, iPad, and Android</li>
    </ul>
  </description>

  <launchable type="desktop-id">net.lockbook.Lockbook.desktop</launchable>

  <supports>
    <control>pointing</control>
    <control>keyboard</control>
    <control>touch</control>
    <control>tablet</control>
  </supports>

  <url type="homepage">https://lockbook.net/</url>
  <url type="bugtracker">https://github.com/lockbook/lockbook/issues</url>
  <url type="vcs-browser">https://github.com/lockbook/lockbook</url>
  <url type="contact">https://discord.gg/lockbook</url>

  <screenshots>
    <screenshot type="default">
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/01-hero.png</image>
      <caption>The best place for your notes</caption>
    </screenshot>
    <screenshot>
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/02-drawing.png</image>
      <caption>Sketch on an infinite canvas</caption>
    </screenshot>
    <screenshot>
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/06-pdf.png</image>
      <caption>Your home for everything</caption>
    </screenshot>
    <screenshot>
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/03-links.png</image>
      <caption>Your notes, connected</caption>
    </screenshot>
    <screenshot>
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/04-together.png</image>
      <caption>Shared only by invitation</caption>
    </screenshot>
    <screenshot>
      <image type="source">https://raw.githubusercontent.com/lockbook/lockbook/{version}/utils/dev/flatpak-package/screenshots/07-keys.png</image>
      <caption>You hold the keys</caption>
    </screenshot>
  </screenshots>

  <branding>
    <color type="primary" scheme_preference="dark">#1c4f80</color>
    <color type="primary" scheme_preference="light">#8fc4f5</color>
  </branding>

  <categories>
    <category>Office</category>
  </categories>

  <keywords>
    <keyword>notes</keyword>
    <keyword>note taking</keyword>
    <keyword>markdown</keyword>
    <keyword>notebook</keyword>
    <keyword>encrypted notes</keyword>
    <keyword>private notes</keyword>
    <keyword>journal</keyword>
  </keywords>

  <content_rating type="oars-1.1" />

  <releases>
    <release version="{version}" date="{today}" />
  </releases>
</component>
"#;
