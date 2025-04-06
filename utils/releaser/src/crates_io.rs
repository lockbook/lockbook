use crate::secrets::CratesIO;
use crate::utils::CommandRunner;
use cli_rs::cli_error::CliResult;
use std::fs;
use std::process::Command;

pub fn release_crate(package: String) -> CliResult<()> {
    let api_token = CratesIO::env();
    let root_path = fs::canonicalize("../../../")?;

    Command::new("cargo")
        .args(["publish", &format!("--token={}", api_token.0), "-p", &package])
        .current_dir(root_path)
        .assert_success();

    Ok(())
}
