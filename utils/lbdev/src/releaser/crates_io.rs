use crate::releaser::secrets::CratesIO;
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use std::process::Command;

pub fn release_crate(package: String) -> CliResult<()> {
    let api_token = CratesIO::env();
    Command::new("cargo")
        .args(["publish", &format!("--token={}", api_token.0), "-p", &package])
        .assert_success();

    Ok(())
}
