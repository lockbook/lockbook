use crate::{CliError, ToolEnvironment};
use execute_command_macro::command;

pub fn fmt_workspace(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let fmt_result = command!("cargo fmt -- --check -l")
        .current_dir(&tool_env.root_dir)
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

pub fn clippy_workspace(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let clippy_result = command!("cargo clippy -- -D warnings")
        .current_dir(&tool_env.root_dir)
        .spawn()?
        .wait()?;

    if !clippy_result.success() {
        return Err(CliError::basic_error());
    }

    let clippy_result = command!("cargo clippy --tests -- -D warnings")
        .current_dir(&tool_env.root_dir)
        .spawn()?
        .wait()?;

    if !clippy_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
