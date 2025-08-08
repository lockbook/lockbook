mod cli;
mod ios;
mod mac;
mod ws;

use cli_rs::cli_error::CliResult;

use std::fs;
use std::path::Path;

pub fn release() -> CliResult<()> {
    cli::release();
    ws::build();
    clean_build_dir();
    ios::release();
    mac::release();
    Ok(())
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
