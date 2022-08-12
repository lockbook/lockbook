use crate::utils::CommandRunner;
use crate::ToolEnvironment;
use std::process::Command;

pub fn fmt_workspace(tool_env: &ToolEnvironment) {
    Command::new("cargo")
        .args(["fmt", "--", "--check", "-l"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}

pub fn clippy_workspace(tool_env: &ToolEnvironment) {
    Command::new("cargo")
        .args(["clippy", "--tests", "--", "-D", "warnings"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
