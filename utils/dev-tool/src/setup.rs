use crate::{utils, ToolEnvironment};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

pub fn install_ci_dependencies(tool_env: ToolEnvironment) {
    install_rust();
    install_apt_packages();

    let add_result = Command::new(tool_env.home_dir.join(".cargo/bin/rustup"))
        .args(&["component", "add", "clippy", "rustfmt"])
        .status()
        .unwrap();

    if !add_result.success() {
        panic!()
    }

    install_android_tooling(
        ANDROID_CLI_LINK,
        ANDROID_CLI_ZIP_NAME,
        &tool_env.sdk_dir.join(ANDROID_CLI_FOLDER_NAME),
    );
    install_android_tooling(
        ANDROID_NDK_LINK,
        ANDROID_NDK_ZIP_NAME,
        &tool_env.sdk_dir.join(ANDROID_NDK_FOLDER_NAME),
    );

    let yes = Command::new("yes").stdout(Stdio::piped()).spawn().unwrap();

    let license_result = Command::new(
        tool_env
            .sdk_dir
            .join(ANDROID_CLI_FOLDER_NAME)
            .join("cmdline-tools/bin/sdkmanager"),
    )
    .args(&[
        "--licenses",
        &format!("--sdk_root={}", tool_env.sdk_dir.join("android-ndk").to_str().unwrap()),
    ])
    .stdin(yes.stdout.unwrap())
    .status()
    .unwrap();

    utils::panic_if_unsuccessful(license_result);
}

fn install_rust() {
    let curl_result = Command::new("curl")
        .args(["--proto", "https", "--tlsv1.2", "-sSf", "https://sh.rustup.rs"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let install_result = Command::new("sh")
        .stdin(curl_result.stdout.unwrap())
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(install_result);
}

fn install_apt_packages() {
    let install_result = Command::new("sudo")
        .args(["apt", "update"])
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(install_result);

    let install_result = Command::new("sudo")
        .args(["apt", "install"].into_iter().chain(DEPENDENCIES))
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(install_result);
}

fn install_android_tooling<P: AsRef<Path>>(link: &str, zip_name: &str, destination: P) {
    let tmp_dir = PathBuf::from("/tmp");

    let download_result = Command::new("wget")
        .args(&["-q", link, "-O", zip_name])
        .current_dir(&tmp_dir)
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(download_result);

    let unzip_result = Command::new("unzip")
        .args(&[zip_name, "-d", destination.as_ref().to_str().unwrap()])
        .current_dir(&tmp_dir)
        .status()
        .unwrap();

    utils::panic_if_unsuccessful(unzip_result);
}
