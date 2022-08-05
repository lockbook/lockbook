use crate::error::CliError;
use crate::{utils, ToolEnvironment};
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

pub fn fmt_android(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let fmt_result = command!("./gradlew lintKotlin")
        .current_dir(utils::android_dir(tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
