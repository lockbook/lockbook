use lb::Core;

use crate::resolve_target_to_file;
use crate::CliError;

#[derive(clap::Subcommand, Debug)]
pub enum DebugCmd {
    /// helps find invalid states within lockbook
    Validate,
    /// print metadata associated with a file
    Info {
        /// lockbook file path or ID
        target: String,
    },
    /// print who is logged into this lockbook
    Whoami,
    /// print information about where this lockbook is stored and its server url
    Whereami,
}

pub fn debug(core: &Core, cmd: DebugCmd) -> Result<(), CliError> {
    match cmd {
        DebugCmd::Validate => validate(core),
        DebugCmd::Info { target } => info(core, &target),
        DebugCmd::Whoami => whoami(core),
        DebugCmd::Whereami => whereami(core),
    }
}

fn validate(core: &Core) -> Result<(), CliError> {
    let warnings = core
        .validate()
        .map_err(|err| CliError(format!("validating: {:?}", err)))?;
    if warnings.is_empty() {
        return Ok(());
    }
    for w in &warnings {
        eprintln!("{:#?}", w);
    }
    Err(CliError(format!("{} warnings found", warnings.len())))
}

fn info(core: &Core, target: &str) -> Result<(), CliError> {
    let f = resolve_target_to_file(core, target)?;
    println!("{:#?}", f);
    Ok(())
}

fn whoami(core: &Core) -> Result<(), CliError> {
    println!("{}", core.get_account()?.username);
    Ok(())
}

fn whereami(core: &Core) -> Result<(), CliError> {
    let account = core.get_account()?;
    let config = &core.get_config()?;
    println!("Server: {}", account.api_url);
    println!("Core: {}", config.writeable_path);
    Ok(())
}
