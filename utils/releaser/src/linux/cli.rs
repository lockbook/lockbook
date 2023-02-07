use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;
use gh_release::ReleaseClient;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

pub fn release(gh: &Github) {
    update_aur();
    update_snap();
    upload(gh);
}

pub fn update_snap() {
    let version = core_version();
    let snap_name = format!("lockbook_{version}_amd64.snap");

    let new_content = format!(
        r#"
name: lockbook
base: core20
version: '{version}'
summary: The CLI version of Lockbook
description: |
  The private, polished note-taking platform.
grade: stable
confinement: strict

parts:
  lockbook:
    plugin: rust
    source: https://github.com/lockbook/lockbook.git
    source-tag: {version}
    build-packages:
      - git
    rust-path: ["clients/cli"]

apps:
  lockbook:
    command: bin/lockbook
    plugs:
      - network
      - home
    "#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("utils/dev/snap-packages/lockbook/snap/snapcraft.yaml")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();

    Command::new("snapcraft")
        .current_dir("utils/dev/snap-packages/lockbook/snap")
        .assert_success();
    Command::new("snapcraft")
        .args(["upload", "--release=stable", &snap_name])
        .current_dir("utils/dev/snap-packages/lockbook/snap")
        .assert_success();
}

pub fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open("target/release/lockbook").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-cli",
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
pkgname='lockbook'
_pkgname="lockbook"
pkgver={version}
pkgrel=1
arch=('x86_64' 'i686')
url="https://github.com/lockbook/lockbook"
pkgdesc="A secure, private, minimal, cross-platform document editor."
license=('BSD-3-Clause')
makedepends=('rust' 'cargo' 'git')
provides=('lockbook')
conflicts=('lockbook')
source=("git+https://github.com/lockbook/lockbook.git#tag=$pkgver")
sha256sums=('SKIP')
groups=('lockbook')

pkgver() {{
  cd $srcdir/lockbook/clients/cli
  echo "{version}"
}}

build() {{
  cd $srcdir/lockbook/clients/cli
  cargo build --release --locked
}}

package_lockbook() {{
  cd $srcdir/lockbook
  install -D -m755 "target/release/lockbook" "$pkgdir/usr/bin/lockbook"
}}
"#
    );

    let new_src_info_content = format!(
        r#"
pkgbase = lockbook
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
	provides = lockbook
	conflicts = lockbook
	source = git+https://github.com/lockbook/lockbook.git
	sha256sums = SKIP

pkgname = lockbook
        "#
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../aur-lockbook/PKGBUILD")
        .unwrap();
    file.write_all(new_makepkg_content.as_bytes()).unwrap();

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../aur-lockbook/.SRCINFO")
        .unwrap();
    file.write_all(new_src_info_content.as_bytes()).unwrap();
}

pub fn push_aur() {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir("../aur-lockbook")
        .assert_success();
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir("../aur-lockbook")
        .assert_success();
    Command::new("git")
        .args(["push", "aur", "master"])
        .current_dir("../aur-lockbook")
        .assert_success();
    Command::new("git")
        .args(["push", "github", "master"])
        .current_dir("../aur-lockbook")
        .assert_success();
}
