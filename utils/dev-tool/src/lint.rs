use crate::error::CliError;
use crate::{utils, ToolEnvironment};
use execute_command_macro::command;

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

pub fn lint_android(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let lint_result = command!("./gradlew lint")
        .current_dir(utils::android_dir(tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !lint_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
