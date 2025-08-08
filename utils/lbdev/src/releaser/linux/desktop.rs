use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

pub fn release() -> CliResult<()> {
    upload_deb()?;
    update_aur()?;
    update_snap()?;
    build_x86()?;
    upload();
    Ok(())
}

pub fn update_snap() -> CliResult<()> {
    let version = lb_version();
    let snap_name = format!("lockbook-desktop_{version}_amd64.snap");

    let new_content = format!(
        r#"
name: lockbook-desktop
base: core22
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
      - cargo
      - git
      - libssl-dev
      - pkg-config
      - cmake
      - libfontconfig1-dev
      - libfontconfig
      - libxkbcommon-x11-dev
    rust-path: ["clients/linux"]
    override-pull: |
      snapcraftctl pull
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source "$HOME/.cargo/env"
      rustup default 1.88

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

pub fn upload_deb() -> CliResult<()> {
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
        .open("utils/releaser/debian-build-scripts/ppa-lockbook/debian/control")
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

    let output = File::open(format!("utils/releaser/debian-build-scripts/{deb_file}")).unwrap();

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
pub fn upload() {
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
}

pub fn update_aur() -> CliResult<()> {
    overwrite_lockbook_pkg();
    push_aur()?;
    Ok(())
}

pub fn overwrite_lockbook_pkg() {
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

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../aur-lockbook-desktop/PKGBUILD")
        .unwrap();
    file.write_all(new_makepkg_content.as_bytes()).unwrap();

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../aur-lockbook-desktop/.SRCINFO")
        .unwrap();
    file.write_all(new_src_info_content.as_bytes()).unwrap();
}

pub fn push_aur() -> CliResult<()> {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success()?;
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success()?;
    Command::new("git")
        .args(["push", "aur", "master"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success()?;
    Command::new("git")
        .args(["push", "github", "master"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success()?;
    Ok(())
}
