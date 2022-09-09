use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use fs_extra::dir::copy;
use fs_extra::dir::CopyOptions;

use crate::utils::{root, CommandRunner};

pub fn release() {
    let mut path = work_dir();

    clone(&mut path);
    checkout(&path);
    remove_old(&path);
    copy_public_site(&path);
    push(&path);
}

fn work_dir() -> PathBuf {
    let loc = tempfile::tempdir().unwrap().into_path();
    println!("operating in {:?}", loc);
    loc
}

fn clone(tmp: &mut PathBuf) {
    Command::new("git")
        .args(["clone", "git@github.com:lockbook/lockbook.git"])
        .current_dir(&tmp)
        .assert_success();
    tmp.push("lockbook");
}

fn checkout(tmp: &Path) {
    Command::new("git")
        .args(["checkout", "gh-pages"])
        .current_dir(tmp)
        .assert_success();
}

fn remove_old(tmp: &Path) {
    Command::new("git")
        .args(["rm", "-rf", "."])
        .current_dir(tmp)
        .assert_success();
}

fn copy_public_site(tmp: &Path) {
    let mut pub_site = root();
    pub_site.push("public_site");

    let cfg = CopyOptions { copy_inside: true, content_only: true, ..CopyOptions::default() };

    copy(pub_site, tmp, &cfg).unwrap();
}
fn push(tmp: &Path) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(tmp)
        .assert_success();

    Command::new("git")
        .args(["commit", "-m", "releaser deploy"])
        .current_dir(tmp)
        .assert_success();

    Command::new("git")
        .args(["push", "origin", "gh-pages"])
        .current_dir(tmp)
        .assert_success();
}
