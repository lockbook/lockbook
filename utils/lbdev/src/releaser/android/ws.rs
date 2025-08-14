use cli_rs::cli_error::CliResult;

use crate::{places::workspace_ffi, utils::CommandRunner};
use std::process::Command;

pub fn build() -> CliResult<()> {
    Command::new("bash")
        .args(["create_android_libs.sh"])
        .current_dir(workspace_ffi())
        .assert_success()
}
