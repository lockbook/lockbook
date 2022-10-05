use crate::utils::{core_version, CommandRunner};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

pub fn update_aur() {
    overwrite_lockbook_pkg();
    push_aur();
}

pub fn overwrite_lockbook_pkg() {
    let version = core_version();

    let new_content = format!(
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
source=("git+https://github.com/lockbook/lockbook.git")
sha256sums=('SKIP')
groups=('lockbook')

pkgver() {{
  cd $srcdir/lockbook/clients/cli
  echo "$(grep '^version =' Cargo.toml|head -n1|cut -d\" -f2|cut -d\- -f1)"
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

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(true)
        .open("../aur-lockbook/PKGBUILD")
        .unwrap();
    file.write_all(new_content.as_bytes()).unwrap();
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
