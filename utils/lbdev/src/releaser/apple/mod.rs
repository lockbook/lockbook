mod cli;
mod ios;
mod mac;

use cli_rs::cli_error::CliResult;

use std::fs;
use std::path::Path;

use crate::local::apple_ws_all;

pub fn release() -> CliResult<()> {
    cli::release()?;
    apple_ws_all()?;
    clean_build_dir();
    ios::release()?;
    mac::release()?;
    Ok(())
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
