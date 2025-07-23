use crate::secrets::Github;
use crate::utils::{CommandRunner, lb_repo, lb_version};
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::fs::File;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn deploy() -> CliResult<()> {
    build_server();
    backup_old_server();
    replace_old_server();
    restart_server();
    check_server_status();
    upload();

    Ok(())
}

fn build_server() {
    println!("Building server");
    Command::new("cargo")
        .args(["build", "-p", "lockbook-server", "--release"])
        .assert_success();
}

fn backup_old_server() {
    println!("Backing up currently running server");
    Command::new("ssh")
        .args(["root@api.prod.lockbook.net", "cp", "/usr/bin/lockbook-server", "/root/old-server"])
        .assert_success()
}

fn replace_old_server() {
    println!("scp'ing new server into /root/new-server");
    Command::new("scp")
        .args(["target/release/lockbook-server", "root@api.prod.lockbook.net:/root/new-server"])
        .assert_success();

    println!("mv new-server /usr/bin");
    Command::new("ssh")
        .args(["root@api.prod.lockbook.net", "mv", "/root/new-server", "/usr/bin/lockbook-server"])
        .assert_success()
}

fn restart_server() {
    println!("starting new server");
    Command::new("ssh")
        .args(["root@api.prod.lockbook.net", "systemctl", "restart", "lockbook-server.service"])
        .assert_success()
}

fn check_server_status() {
    thread::sleep(Duration::from_secs(5));
    println!("checking on server status");
    Command::new("curl")
        .args(["https://api.prod.lockbook.net/get-build-info"])
        .assert_success()
}

fn upload() {
    let gh = Github::env();
    let client = ReleaseClient::new(gh.0).unwrap();
    let release = client
        .get_release_by_tag_name(&lb_repo(), &lb_version())
        .unwrap();

    let file = File::open("target/release/lockbook-server").unwrap();
    client
        .upload_release_asset(
            &lb_repo(),
            release.id,
            "lockbook-server",
            "application/octet-stream",
            file,
            None,
        )
        .unwrap();
}
