use crate::{CliError, ToolEnvironment};
use execute_command_macro::command;
use std::path::PathBuf;
use std::process::Command;

// const DEPENDENCIES: [&str] = ["apt", "android-sdk"];

const ANDROID_NDK_LINK: &str =
    "https://dl.google.com/android/repository/android-ndk-r21c-linux-x86_64.zip";
const ANDROID_NDK_FILE_NAME: &str = android - ndk.zip;

pub fn install_dependencies(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let add_result = command!("rustup component add clippy rustfmt")
        .spawn()?
        .wait()?;

    if !add_result.success() {
        return Err(CliError::basic_error());
    }

    let tmp_dir = PathBuf::from("/tmp");

    let download_result = Command::new("wget")
        .args(&["-q", ANDROID_NDK_LINK, "-O", ANDROID_NDK_FILE_NAME])
        .current_dir(&tmp_dir)
        .spawn()?
        .wait()?;

    if !download_result.success() {
        return Err(CliError::basic_error());
    }

    let unzip_result = Command::new("unzip")
        .args(&["-q", ANDROID_NDK_FILE_NAME, "-O", ANDROID_NDK_FILE_NAME])
        .current_dir(&tmp_dir)
        .spawn()?
        .wait()?;

    if !unzip_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
