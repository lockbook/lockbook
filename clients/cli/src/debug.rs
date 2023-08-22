use cli_rs::cli_error::{CliError, CliResult};
use lb::Core;

use crate::{ensure_account, input::FileInput};

pub fn validate(core: &Core) -> CliResult<()> {
    ensure_account(core)?;

    let warnings = core
        .validate()
        .map_err(|err| CliError::from(format!("validating: {:?}", err)))?;
    if warnings.is_empty() {
        return Ok(());
    }
    for w in &warnings {
        eprintln!("{:#?}", w);
    }
    Err(CliError::from(format!("{} warnings found", warnings.len())))
}

pub fn info(core: &Core, target: FileInput) -> Result<(), CliError> {
    ensure_account(core)?;

    let f = target.find(core)?;
    println!("{:#?}", f);
    Ok(())
}

pub fn whoami(core: &Core) -> Result<(), CliError> {
    ensure_account(core)?;

    println!("{}", core.get_account()?.username);
    Ok(())
}

pub fn whereami(core: &Core) -> Result<(), CliError> {
    ensure_account(core)?;

    let account = core.get_account()?;
    let config = &core.get_config()?;
    println!("Server: {}", account.api_url);
    println!("Core: {}", config.writeable_path);
    Ok(())
}
