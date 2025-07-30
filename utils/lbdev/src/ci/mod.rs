use std::process::Command;

use cli_rs::cli_error::CliResult;

use crate::utils::CommandRunner;

pub(crate) fn clippy() -> CliResult<()> {
    Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(&tool_env.root_dir)
        .assert_success();

    Ok(())
}
