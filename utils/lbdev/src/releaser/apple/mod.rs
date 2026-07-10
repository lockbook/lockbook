pub mod cli;
pub mod ios;
pub mod mac;

use cli_rs::cli_error::CliResult;
use time::OffsetDateTime;

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
    let now = OffsetDateTime::now_utc();

    let year = now.year();
    let month = now.month() as u8;
    let day = now.day();
    let hour = now.hour();
    let minute = now.minute();
    let second = now.second();

    format!("{year}{month:0>2}{day:0>2}.{hour:0>2}{minute:0>2}{second:0>2}")
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
