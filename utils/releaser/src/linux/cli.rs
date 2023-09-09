use crate::utils::{core_version, lb_repo, CommandRunner};
use crate::Github;
use gh_release::ReleaseClient;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

pub fn release(gh: &Github) {
    update_aur();
    update_snap();
    build_x86();
    upload(gh);
}

pub fn build_x86() {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-cli", "--release", "--target=x86_64-unknown-linux-gnu"])
        .assert_success();
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

    fs::create_dir_all("utils/dev/snap-packages/lockbook/snap").unwrap();

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("utils/dev/snap-packages/lockbook/snap/snapcraft.yaml")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();

    Command::new("snapcraft")
        .current_dir("utils/dev/snap-packages/lockbook/")
        .assert_success();
    Command::new("snapcraft")
        .args(["upload", "--release=stable", &snap_name])
        .current_dir("utils/dev/snap-packages/lockbook/")
        .assert_success();
}

pub fn upload(gh: &Github) {
    let client = ReleaseClient::new(gh.0.clone()).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &core_version())
        .unwrap();
    let file = File::open("target/x86_64-unknown-linux-gnu/release/lockbook").unwrap();
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
arch=('any')
url="https://github.com/lockbook/lockbook"
pkgdesc="The private, polished note-taking platform."
license=('BSD-3-Clause')
makedepends=('cargo' 'git')
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

  lockbook completions bash > lockbook_completions.bash
  lockbook completions zsh > lockbook_completions.zsh
  lockbook completions fish > lockbook_completions.fish

  install -Dm644 lockbook_completions.bash "$pkgdir/usr/share/bash-completion/completions/lockbook"
  install -Dm644 lockbook_completions.zsh "$pkgdir/usr/share/zsh/site-functions/_lockbook"
  install -Dm644 lockbook_completions.fish "$pkgdir/usr/share/fish/vendor_completions.d/lockbook.fish"
}}
"#
    );

    let new_src_info_content = format!(
        r#"
pkgbase = lockbook
	pkgdesc = The private, polished note-taking platform.
	pkgver = {version}
	pkgrel = 1
	url = https://github.com/lockbook/lockbook
	arch = any
	groups = lockbook
	license = BSD-3-Clause
	makedepends = cargo
	makedepends = git
	provides = lockbook
	conflicts = lockbook
	source = git+https://github.com/lockbook/lockbook.git#tag=v{version}
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
