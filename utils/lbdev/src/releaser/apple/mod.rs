pub mod cli;
pub mod ios;
pub mod mac;

use chrono_tz::America::New_York;
use cli_rs::cli_error::CliResult;
use google_androidpublisher3::chrono::Utc;

use std::fs;
use std::path::Path;

use crate::local::apple_ws_all;

pub fn release() -> CliResult<()> {
    cli::release()?;
    apple_ws_all()?;
    clean_build_dir();
    ios::release(false)?;
    mac::release(false, true, true)?;
    Ok(())
}

pub fn build_number() -> String {
    Utc::now()
        .with_timezone(&New_York)
        .format("%Y%m%d.%H%M%S")
        .to_string()
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
