use crate::{CliError, ToolEnvironment};
use execute_command_macro::command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

//       - libgtksourceview-5-dev
//       - libgspell-1-dev
const DEPENDENCIES: [&str; 9] = [
    "curl",
    "unzip",
    "wget",
    "openjdk-11-jdk",
    "cbindgen",
    "swift",
    "build-essential",
    "libgtksourceview-5-dev",
    "libgspell-1-dev",
];

const ANDROID_CLI_LINK: &str =
    "https://dl.google.com/android/repository/commandlinetools-linux-7583922_latest.zip";
const ANDROID_CLI_ZIP_NAME: &str = "android-cli-tools.zip";
pub const ANDROID_CLI_FOLDER_NAME: &str = "android-cli-tools";

const ANDROID_NDK_LINK: &str =
    "https://dl.google.com/android/repository/android-ndk-r21c-linux-x86_64.zip";
const ANDROID_NDK_ZIP_NAME: &str = "android-ndk.zip";
pub const ANDROID_NDK_FOLDER_NAME: &str = "android-ndk";

pub fn install_ci_dependencies(tool_env: ToolEnvironment) -> Result<(), CliError> {
    install_rust()?;
    install_apt_packages()?;

    let add_result = Command::new(tool_env.home_dir.join(".cargo/bin/rustup"))
        .args(&["component", "add", "clippy", "rustfmt"])
        .status()?;

    if !add_result.success() {
        return Err(CliError::basic_error());
    }

    install_android_tooling(
        ANDROID_CLI_LINK,
        ANDROID_CLI_ZIP_NAME,
        &tool_env.sdk_dir.join(ANDROID_CLI_FOLDER_NAME),
    )?;
    install_android_tooling(
        ANDROID_NDK_LINK,
        ANDROID_NDK_ZIP_NAME,
        &tool_env.sdk_dir.join(ANDROID_NDK_FOLDER_NAME),
    )?;

    let mut yes = Command::new("yes").stdout(Stdio::piped()).spawn()?;

    let license_result = Command::new(
        tool_env
            .sdk_dir
            .join(ANDROID_CLI_FOLDER_NAME)
            .join("cmdline-tools/bin/sdkmanager"),
    )
    .args(&[
        "--licenses",
        &format!(
            "--sdk_root={}",
            tool_env
                .sdk_dir
                .join("android-ndk")
                .to_str()
                .ok_or(CliError::basic_error())?
        ),
    ])
    .stdin(yes.stdout.ok_or(CliError::basic_error())?)
    .status()?;

    if !license_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

fn install_rust() -> Result<(), CliError> {
    let install_result = Command::new("curl")
        .args(["--proto", "https", "--tlsv1.2", "-sSf", "https://sh.rustup.rs"])
        .stdout(Stdio::piped())
        .spawn()?;

    let install_result = Command::new("sh")
        .stdin(install_result.stdout.ok_or(CliError::basic_error())?)
        .status()?;

    if !install_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

fn install_apt_packages() -> Result<(), CliError> {
    let install_result = command!("sudo apt update").status().unwrap();

    if !install_result.success() {
        return Err(CliError(Some("Failed to update packages".to_string())));
    }

    let install_result = Command::new("sudo")
        .args(["apt", "install"].into_iter().chain(DEPENDENCIES))
        .status()?;

    if !install_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

fn install_android_tooling<P: AsRef<Path>>(
    link: &str, zip_name: &str, destination: P,
) -> Result<(), CliError> {
    let tmp_dir = PathBuf::from("/tmp");

    let download_result = Command::new("wget")
        .args(&["-q", link, "-O", zip_name])
        .current_dir(&tmp_dir)
        .spawn()?
        .wait()?;

    if !download_result.success() {
        return Err(CliError::basic_error());
    }

    let unzip_result = Command::new("unzip")
        .args(&[
            zip_name,
            "-d",
            destination
                .as_ref()
                .to_str()
                .ok_or(CliError::basic_error())?,
        ])
        .current_dir(&tmp_dir)
        .spawn()?
        .wait()?;

    if !unzip_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
