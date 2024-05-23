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
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}

pub fn udeps_workspace(tool_env: &ToolEnvironment) {
    Command::new("cargo")
        .args(["+nightly", "udeps", "--all-targets"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}

pub fn assert_git_clean(tool_env: &ToolEnvironment) {
    Command::new("git")
        .args(["diff", "--exit-code"])
        .current_dir(&tool_env.root_dir)
        .assert_success();
}
