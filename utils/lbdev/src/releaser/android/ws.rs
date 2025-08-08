use cli_rs::cli_error::CliResult;

use crate::utils::CommandRunner;
use std::process::Command;

pub fn build() -> CliResult<()> {
    Command::new("bash")
        .args(["create_android_libs.sh"])
        .current_dir("libs/content/workspace-ffi")
        .assert_success()
}
