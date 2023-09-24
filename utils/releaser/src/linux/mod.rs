use cli_rs::cli_error::CliResult;

mod cli;
mod desktop;

pub fn release() -> CliResult<()> {
    cli::release();
    desktop::release();

    Ok(())
}
