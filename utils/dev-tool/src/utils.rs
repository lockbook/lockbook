use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::CliError;

pub fn in_android_dir(command: &mut Command) -> Result<(), CliError>{
    let android_dir = std::env::current_dir()?
        .join("clients/android");

    command
        .current_dir(android_dir);

    Ok(())
}

pub fn in_server_dir(command: &mut Command) -> Result<(), CliError>{

    let android_dir = std::env::current_dir()?
        .join("server/server");

    command
        .current_dir(android_dir);

    Ok(())
}

pub fn local_env_path<P: AsRef<Path>>(root_dir: P) -> PathBuf {
    root_dir
        .as_ref()
        .join("containers")
}
