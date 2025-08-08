pub mod cli;
pub mod desktop;

use std::fs;
use std::path::Path;

use cli_rs::cli_error::CliResult;

pub fn release() -> CliResult<()> {
    let build_dir = Path::new("windows-build");
    if !build_dir.exists() {
        fs::create_dir("windows-build").unwrap();
    }
    cli::release()?;
    desktop::release()?;

    fs::remove_dir_all("windows-build").unwrap();
    Ok(())
}
