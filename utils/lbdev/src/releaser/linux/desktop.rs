use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub fn release() -> CliResult<()> {
    update_aur()?;
    update_snap()?;
    upload_gh()?;
    update_flatpak()?;
    Ok(())
}

pub fn update_snap() -> CliResult<()> {
    let version = lb_version();
    let snap_name = format!("lockbook-desktop_{version}_amd64.snap");

    let new_content = format!(
        r#"
name: lockbook-desktop
base: core24
version: '{version}'
summary: The linux gui version of Lockbook
description: |
  The private, polished note-taking platform.
grade: stable
confinement: strict

parts:
  lockbook-desktop:
    plugin: rust
    source: https://github.com/lockbook/lockbook.git
    source-tag: {version}
    build-packages:
      - git
      - libssl-dev
      - pkg-config
      - cmake
      - libfontconfig1-dev
      - libfontconfig
      - libxkbcommon-x11-dev
    stage-packages:
      - libxkbcommon-x11-0
      - libfontconfig1
    rust-path: ["clients/linux"]

apps:
  lockbook-desktop:
    command: bin/lockbook-linux
    extensions: [gnome]
    plugs:
      - network
      - home
    "#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("utils/dev/snap-packages/lockbook-desktop/snap/snapcraft.yaml")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();

    Command::new("snapcraft")
        .args(["pack", "--destructive-mode"])
        .current_dir("utils/dev/snap-packages/lockbook-desktop/")
        .assert_success()?;
    Command::new("snapcraft")
        .args(["upload", "--release=stable", &snap_name])
        .current_dir("utils/dev/snap-packages/lockbook-desktop/")
        .assert_success()?;
    Ok(())
}

pub fn build_x86() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-linux", "--release", "--target=x86_64-unknown-linux-gnu"])
        .assert_success()
}

pub fn upload_deb_gh() -> CliResult<()> {
    let lb_version = &lb_version();
    let gh = Github::env();

    let deb_scripts_location =
        "utils/lbdev/src/releaser/debian-build-scripts/ppa-lockbook-desktop/";
    Command::new("dch")
        .args([
            "--newversion",
            lb_version,
            "see changelog at https://github.com/lockbook/lockbook/releases/latest",
        ])
        .current_dir(deb_scripts_location)
        .env("DEBEMAIL", "Parth<parth@mehrotra.me>")
        .assert_success()?;

    let new_control = format!(
        r#"
Source: lockbook-desktop
Section: utils
Priority: extra
Maintainer: Parth Mehrotra <parth@mehrotra.me>
Standards-Version: {lb_version}
Build-Depends: debhelper (>=10), git, ca-certificates, libxkbcommon

Package: lockbook-desktop
Architecture: any
Depends: ${{shlibs:Depends}}, ${{misc:Depends}}
Description: The private, polished note-taking platform.
"#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("utils/lbdev/src/releaser/debian-build-scripts/ppa-lockbook/debian/control")
        .unwrap();
    file.write_all(new_control.as_bytes()).unwrap();

    Command::new("debuild")
        .current_dir(deb_scripts_location)
        .assert_success()?;

    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), lb_version)
        .unwrap();

    let deb_file = format!("lockbook_{lb_version}_amd64.deb");

    let output =
        File::open(format!("utils/lbdev/src/releaser/debian-build-scripts/{deb_file}")).unwrap();

    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            &deb_file,
            "application/octet-stream",
            output,
            None,
        )
        .unwrap();
    Ok(())
}
pub fn upload_gh() -> CliResult<()> {
    build_x86()?;
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();
    let file = File::open("target/x86_64-unknown-linux-gnu/release/lockbook-linux").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-linux",
            "application/octet-stream",
            file,
            None,
        )
        .unwrap();
    Ok(())
}

pub fn update_aur() -> CliResult<()> {
    let temp_dir = env::temp_dir().join("aur-lockbook-desktop");

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    clone_aur_repo(&temp_dir)?;
    overwrite_lockbook_pkg(&temp_dir);
    push_aur(&temp_dir)?;

    fs::remove_dir_all(&temp_dir).unwrap();
    Ok(())
}

fn clone_aur_repo(temp_dir: &Path) -> CliResult<()> {
    Command::new("git")
        .args([
            "clone",
            "ssh://aur@aur.archlinux.org/lockbook-desktop.git",
            temp_dir.to_str().unwrap(),
        ])
        .assert_success()
}

fn overwrite_lockbook_pkg(temp_dir: &Path) {
    let version = lb_version();

    let new_makepkg_content = format!(
        r#"
pkgname='lockbook-desktop'
_pkgname="lockbook-desktop"
pkgver={version}
pkgrel=1
arch=('x86_64' 'i686')
url="https://github.com/lockbook/lockbook"
pkgdesc="The private, polished note-taking platform."
license=('BSD-3-Clause')
makedepends=('cargo' 'git' 'cmake' 'base-devel' 'gtk3')
depends=()
provides=('lockbook-desktop')
conflicts=('lockbook-desktop')
source=("git+https://github.com/lockbook/aur-lockbook-desktop.git" "git+https://github.com/lockbook/lockbook.git#tag=$pkgver")
sha256sums=('SKIP' 'SKIP')
groups=('lockbook')
options=(!lto)

pkgver() {{
  echo "{version}"
}}

build() {{
  echo $_pkgname
  cd $srcdir/lockbook/clients/linux
  cargo build --release --locked
}}

package() {{
  install -D -m755 "$srcdir/lockbook/target/release/lockbook-linux" "$pkgdir/usr/bin/lockbook-desktop"
  install -D -m644 "$srcdir/aur-lockbook-desktop/logo.svg" "$pkgdir/usr/share/pixmaps/logo.svg"
  install -D -m644 "$srcdir/aur-lockbook-desktop/lockbook-desktop.desktop" "$pkgdir/usr/share/applications/lockbook-desktop.desktop"
}}
"#
    );

    let new_src_info_content = format!(
        r#"
pkgbase = lockbook-desktop
	pkgdesc = The private, polished note-taking platform.
	pkgver = {version}
	pkgrel = 1
	url = https://github.com/lockbook/lockbook
	arch = any
	groups = lockbook
	license = BSD-3-Clause
	makedepends = cargo
	makedepends = git
	makedepends = cmake
	makedepends = base-devel
	provides = lockbook-desktop
	conflicts = lockbook-desktop
	source = git+https://github.com/lockbook/aur-lockbook-desktop.git
	source = git+https://github.com/lockbook/lockbook.git#tag=v{version}
	sha256sums = SKIP
	sha256sums = SKIP

pkgname = lockbook-desktop
        "#
    );

    let pkgbuild_path = temp_dir.join("PKGBUILD");
    let mut file = File::create(pkgbuild_path).unwrap();
    file.write_all(new_makepkg_content.as_bytes()).unwrap();

    let srcinfo_path = temp_dir.join(".SRCINFO");
    let mut file = File::create(srcinfo_path).unwrap();
    file.write_all(new_src_info_content.as_bytes()).unwrap();
}

fn push_aur(temp_dir: &Path) -> CliResult<()> {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(temp_dir)
        .assert_success()?;
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir(temp_dir)
        .assert_success()?;
    Command::new("git")
        .args(["push", "origin", "master"])
        .current_dir(temp_dir)
        .assert_success()
}

pub fn overwrite_flatpak_manifest(
    url: &str, sha256: &str, manifest_location: &str,
) -> CliResult<()> {
    let manifest = format!(
        r#"{{
    "app-id": "net.lockbook.Lockbook",
    "runtime": "org.freedesktop.Platform",
    "runtime-version": "25.08",
    "sdk": "org.freedesktop.Sdk",
    "sdk-extensions": [
        "org.freedesktop.Sdk.Extension.rust-stable"
    ],
    "build-options": {{
        "append-path": "/usr/lib/sdk/rust-stable/bin"
    }},
    "command": "lockbook-desktop",
    "finish-args": [
       "--socket=x11",
       "--device=dri",
       "--share=network",
       "--share=ipc"
    ],
    "modules": [
        {{
            "name": "lockbook",
            "buildsystem": "simple",
            "build-options": {{
                "env": {{
                    "CARGO_HOME": "/run/build/lockbook/cargo"
                }}
            }},
            "build-commands": [
                "cargo build --release --offline -p lockbook-linux",
                "install -Dm755 target/release/lockbook-linux /app/bin/lockbook-desktop",
                "install -Dm644 docs/graphics/logo.svg /app/share/icons/hicolor/scalable/apps/net.lockbook.Lockbook.svg",
                "install -Dm644 utils/dev/flatpak-package/lockbook-desktop.desktop /app/share/applications/net.lockbook.Lockbook.desktop",
                "install -Dm644 utils/dev/flatpak-package/net.lockbook.Lockbook.appdata.xml /app/share/appdata/net.lockbook.Lockbook.appdata.xml",
                "install -Dm0644 UNLICENSE -t /app/share/licenses/net.lockbook.Lockbook/"
            ],
            "sources": [
                {{
                    "type": "archive",
                    "url": "{url}",
                    "sha256": "{sha256}"
                }},
                "cargo-sources.json"
            ]
        }}
    ]
}}"#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(manifest_location)
        .unwrap();
    file.write_all(manifest.as_bytes()).unwrap();
    Ok(())
}

pub fn update_flatpak() -> CliResult<()> {
    let version = lb_version();
    let flatpak_builder_tools_directory = "/tmp/flatpak_builder_tools_directory".to_string();
    let released_lb_tarball_url =
        format!("https://github.com/lockbook/lockbook/archive/refs/tags/{version}.tar.gz");
    let released_lb_tarball_dl_location = format!("/tmp/released_lb_tarball-{version}.tar.gz");
    let flatpak_repo_directory = "/tmp/lb_flatpak_repo".to_string();

    if Path::new(&flatpak_builder_tools_directory).exists() {
        fs::remove_dir_all(&flatpak_builder_tools_directory).unwrap();
    }
    if Path::new(&flatpak_repo_directory).exists() {
        fs::remove_dir_all(&flatpak_repo_directory).unwrap();
    }

    Command::new("git")
        .args([
            "clone",
            "--depth=1",
            "https://github.com/flatpak/flatpak-builder-tools.git",
            &flatpak_builder_tools_directory,
        ])
        .assert_success()?;

    Command::new("git")
        .args([
            "clone",
            "--depth=1",
            &format!("https://parth:{}@github.com/flathub/net.lockbook.Lockbook", Github::env().0),
            &flatpak_repo_directory,
        ])
        .assert_success()?;

    Command::new("curl")
        .args(["-fL", &released_lb_tarball_url, "-o", &released_lb_tarball_dl_location])
        .assert_success()?;

    let sha256_output = Command::new("sh")
        .args(["-c", &format!("sha256sum {released_lb_tarball_dl_location} | awk '{{print $1}}'")])
        .output()
        .unwrap();
    let sha256 = String::from_utf8(sha256_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    overwrite_flatpak_manifest(
        &released_lb_tarball_url,
        &sha256,
        &format!("{flatpak_repo_directory}/net.lockbook.Lockbook.json"),
    )?;

    let lock_file_location = Path::new("Cargo.lock")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    Command::new("python3")
        .args([
            "flatpak-cargo-generator.py",
            &lock_file_location,
            "-o",
            &format!("{flatpak_repo_directory}/cargo-sources.json"),
        ])
        .current_dir(format!("{flatpak_builder_tools_directory}/cargo"))
        .assert_success()?;

    push_flatpak(&version, &flatpak_repo_directory)?;

    Ok(())
}

pub fn push_flatpak(version: &str, flatpak_repo: &str) -> CliResult<()> {
    Command::new("git")
        .args(["checkout", "-b", version])
        .current_dir(flatpak_repo)
        .assert_success()?;
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(flatpak_repo)
        .assert_success()?;
    Command::new("git")
        .args(["commit", "-m", &format!("release {}", version)])
        .current_dir(flatpak_repo)
        .assert_success()?;
    Command::new("git")
        .args(["push", "origin", version])
        .current_dir(flatpak_repo)
        .assert_success()?;
    Command::new("gh")
        .args([
            "pr",
            "create",
            "--title",
            &format!("update to {}", version),
            "--body",
            "",
            "--base",
            "master",
            "--head",
            version,
            "--repo",
            "flathub/net.lockbook.Lockbook",
        ])
        .current_dir(flatpak_repo)
        .assert_success()?;
    Ok(())
}
