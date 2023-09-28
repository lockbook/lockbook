use cli_rs::cli_error::CliResult;

pub mod cli;
pub mod desktop;

pub fn release() -> CliResult<()> {
    cli::release()?;
    desktop::release()?;

    Ok(())
}
