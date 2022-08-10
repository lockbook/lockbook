use crate::{panic_if_unsuccessful, ToolEnvironment};
use std::process::Command;

pub fn fmt_workspace(tool_env: ToolEnvironment) {
    let fmt_result = Command::new("cargo")
        .args(["fmt", "--", "--check", "-l"])
        .current_dir(&tool_env.root_dir)
        .status()
        .unwrap();

    panic_if_unsuccessful!(fmt_result);
}

pub fn clippy_workspace(tool_env: ToolEnvironment) {
    let clippy_result = Command::new("cargo")
        .args(["clippy", "--", "-D", "warnings"])
        .current_dir(&tool_env.root_dir)
        .status()
        .unwrap();

    panic_if_unsuccessful!(clippy_result);

    let clippy_result = Command::new("cargo")
        .args(["clippy", "--tests", "--", "-D", "warnings"])
        .current_dir(&tool_env.root_dir)
        .status()
        .unwrap();

    panic_if_unsuccessful!(clippy_result);
}
