use lockbook_core::Core;

use crate::error::CliError;

pub fn whoami(core: &Core) -> Result<(), CliError> {
    println!("{}", core.get_account()?.username);
    Ok(())
}
