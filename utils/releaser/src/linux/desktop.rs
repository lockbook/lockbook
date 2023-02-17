use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;
use gh_release::ReleaseClient;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

pub fn release(gh: &Github) {
    update_aur();
    update_snap();
    build_x86();
    upload(gh);
}

pub fn update_snap() {
    let version = core_version();
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
    rust-path: ["clients/egui"]

apps:
  lockbook-desktop:
    command: bin/lockbook-egui
    extensions: [gnome]
    plugs:
      - network
      - home
    "#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("utils/dev/snap-packages/lockbook-desktop/snap/snapcraft.yaml")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();

    Command::new("snapcraft")
        .current_dir("utils/dev/snap-packages/lockbook-desktop/")
        .assert_success();
    Command::new("snapcraft")
        .args(["upload", "--release=stable", &snap_name])
        .current_dir("utils/dev/snap-packages/lockbook-desktop/")
        .assert_success();
}

pub fn build_x86() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-egui", "--release", "--target=x86_64-unknown-linux-gnu"])
        .assert_success();
}

pub fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open("target/x86_64-unknown-linux-gnu/lockbook-egui").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-egui",
            "application/octet-stream",
            file,
            None,
        )
        .unwrap();
}

pub fn update_aur() {
    overwrite_lockbook_pkg();
    push_aur();
}

pub fn overwrite_lockbook_pkg() {
    let version = core_version();

    let new_makepkg_content = format!(
        r#"
pkgname='lockbook-desktop'
_pkgname="lockbook-desktop"
pkgver={version}
pkgrel=1
arch=('x86_64' 'i686')
url="https://github.com/lockbook/lockbook"
pkgdesc="The best place to store and share thoughts."
license=('BSD-3-Clause')
makedepends=('rust' 'cargo' 'git')
depends=()
provides=('lockbook-desktop')
conflicts=('lockbook-desktop')
source=("git+https://github.com/lockbook/aur-lockbook-desktop.git" "git+https://github.com/lockbook/lockbook.git")
sha256sums=('SKIP' 'SKIP')
groups=('lockbook')

pkgver() {{
  echo "{version}"
}}

build() {{
  echo $_pkgname
  cd $srcdir/lockbook/clients/egui
  rustup update stable
  cargo build --release --locked
}}

package() {{
  install -D -m755 "$srcdir/lockbook/target/release/lockbook-egui" "$pkgdir/usr/bin/lockbook-desktop"
  install -D -m644 "$srcdir/lockbook/graphics/SVGs/dark-1.svg" "$pkgdir/usr/share/pixmaps/dark-1.svg"
  install -D -m644 "$srcdir/aur-lockbook-desktop/lockbook-desktop.desktop" "$pkgdir/usr/share/applications/lockbook-desktop.desktop"
}}
"#
    );

    let new_src_info_content = format!(
        r#"
pkgbase = lockbook-desktop
	pkgdesc = A secure, private, minimal, cross-platform document editor.
	pkgver = {version}
	pkgrel = 1
	url = https://github.com/lockbook/lockbook
	arch = x86_64
	arch = i686
	groups = lockbook
	license = BSD-3-Clause
	makedepends = rust
	makedepends = cargo
	makedepends = git
	provides = lockbook-desktop
	conflicts = lockbook-desktop
	source = git+https://github.com/lockbook/aur-lockbook-desktop.git
	source = git+https://github.com/lockbook/lockbook.git
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

pub fn push_aur() {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success();
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success();
    Command::new("git")
        .args(["push", "aur", "master"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success();
    Command::new("git")
        .args(["push", "github", "master"])
        .current_dir("../aur-lockbook-desktop")
        .assert_success();
}
