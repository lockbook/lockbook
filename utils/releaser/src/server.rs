use crate::utils::{edit_cargo_version, CommandRunner};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn deploy_server(version: &str) {
    edit_cargo_version("server/server/", version);
    build_server();
    backup_old_server();
    replace_old_server();
    restart_server();
    check_server_status();
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
