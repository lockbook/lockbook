use std::process::Command;
use crate::error::CliError;
use crate::utils;

pub fn clippy_workspace() -> Result<(), CliError>{
    let clippy_result = Command::new("cargo")
        .args(&["clippy", "--", "-D", "warnings"])
        .spawn()?
        .wait()?;

    if !clippy_result.success() {
        return Err(CliError::basic_error())
    }

    let clippy_result = Command::new("cargo")
        .args(&["clippy", "--tests", "--", "-D", "warnings"])
        .spawn()?
        .wait()?;

    if !clippy_result.success() {
        return Err(CliError::basic_error())
    }

    Ok(())
}

pub fn lint_android() -> Result<(), CliError> {
    let mut command = Command::new("./gradlew");

    utils::in_android_dir(&mut command)?;

    let lint_result = command
        .arg("lint")
        .spawn()?
        .wait()?;

    if !lint_result.success() {
        return Err(CliError::basic_error())
    }

    Ok(())
}

pub fn lint_apple() -> Result<(), CliError> {

}
