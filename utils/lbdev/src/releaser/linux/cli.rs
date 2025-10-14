use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::Command;

pub fn release() -> CliResult<()> {
    upload_deb()?;
    bin_gh()?;
    update_aur()?;
    update_snap()?;
    Ok(())
}

pub fn build_x86() -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook", "--release", "--target=x86_64-unknown-linux-gnu"])
        .assert_success()
}

pub fn update_snap() -> CliResult<()> {
    let version = lb_version();
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
        .assert_success()?;
    Command::new("snapcraft")
        .args(["upload", "--release=stable", &snap_name])
        .current_dir("utils/dev/snap-packages/lockbook/")
        .assert_success()?;

    Ok(())
}

pub fn upload_deb() -> CliResult<()> {
    let lb_version = &lb_version();
    let gh = Github::env();

    let deb_scripts_location = "utils/lbdev/src/releaser/debian-build-scripts/ppa-lockbook/";
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
Source: lockbook
Section: utils
Priority: extra
Maintainer: Parth Mehrotra <parth@mehrotra.me>
Standards-Version: {lb_version}
Build-Depends: debhelper (>=10), git, ca-certificates

Package: lockbook
Architecture: any
Depends: ${{shlibs:Depends}}, ${{misc:Depends}}, nfs-common
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

pub fn bin_gh() -> CliResult<()> {
    build_x86()?;
    let gh = Github::env();

    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();
    let file = File::open("target/x86_64-unknown-linux-gnu/release/lockbook").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-cli-linux",
            "application/octet-stream",
            file,
            None,
        )
        .unwrap();
    Ok(())
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
options=(!lto)
depends=('nfs-utils')

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

  $pkgdir/usr/bin/lockbook completions bash > lockbook_completions.bash
  $pkgdir/usr/bin/lockbook completions zsh > lockbook_completions.zsh
  $pkgdir/usr/bin/lockbook completions fish > lockbook_completions.fish

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
	depends = nfs-utils
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

pub fn push_aur() -> CliResult<()> {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir("../aur-lockbook")
        .assert_success()?;
    Command::new("git")
        .args(["commit", "-m", "releaser update"])
        .current_dir("../aur-lockbook")
        .assert_success()?;
    Command::new("git")
        .args(["push", "aur", "master"])
        .current_dir("../aur-lockbook")
        .assert_success()?;
    Command::new("git")
        .args(["push", "github", "master"])
        .current_dir("../aur-lockbook")
        .assert_success()?;

    Ok(())
}
