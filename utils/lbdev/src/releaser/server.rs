use crate::places::root;
use crate::releaser::secrets::Github;
use crate::releaser::utils::{lb_repo, lb_version};
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use gh_release::ReleaseClient;
use std::fs::File;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn deploy() -> CliResult<()> {
    build_server()?;
    backup_old_server()?;
    replace_old_server()?;
    restart_server()?;
    check_server_status()?;
    upload();

    Ok(())
}

fn build_server() -> CliResult<()> {
    println!("Building server");
    Command::new("cargo")
        .args(["build", "-p", "lockbook-server", "--release"])
        .current_dir(root())
        .assert_success()
}

fn backup_old_server() -> CliResult<()> {
    println!("Backing up currently running server");
    Command::new("gcloud")
        .args([
            "compute",
            "ssh",
            "--zone",
            "us-east4-c",
            "lb-prod",
            "--project",
            "lockbook-net",
            "--command",
            "cp /usr/bin/lockbook-server ~/lockbook-server.bak",
        ])
        .assert_success()
}

fn replace_old_server() -> CliResult<()> {
    println!("scp'ing new server into /root/new-server");
    Command::new("gcloud")
        .args([
            "compute",
            "scp",
            "--zone",
            "us-east4-c",
            "--project",
            "lockbook-net",
            "target/release/lockbook-server",
            "lb-prod:~/lockbook-server.tmp",
        ])
        .current_dir(root())
        .assert_success()?;

    println!("mv new-server /usr/bin");
    Command::new("gcloud")
        .args([
            "compute",
            "ssh",
            "--zone",
            "us-east4-c",
            "--project",
            "lockbook-net",
            "lb-prod",
            "--command",
            "sudo mv ~/lockbook-server.tmp /root/new-server",
        ])
        .assert_success()?;

    Ok(())
}

fn restart_server() -> CliResult<()> {
    println!("starting new server");
    Command::new("gcloud")
        .args([
            "compute",
            "ssh",
            "--zone",
            "us-east4-c",
            "--project",
            "lockbook-net",
            "lb-prod",
            "--command",
            "sudo systemctl restart lockbook-server.service",
        ])
        .assert_success()
}

fn check_server_status() -> CliResult<()> {
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
