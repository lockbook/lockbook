use std::process::Command;
use crate::error::{CliError};
use crate::utils;

pub fn fmt_workspace() -> Result<(), CliError>{
    let fmt_result = Command::new("cargo")
        .args(&["fmt", "--", "--check", "-l"])
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error())
    }

    Ok(())
}

pub fn fmt_android() -> Result<(), CliError> {
    let mut command = Command::new("./gradlew");

    utils::in_android_dir(&mut command)?;

    let fmt_result = command
        .arg("lintKotlin")
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error())
    }

    Ok(())
}
