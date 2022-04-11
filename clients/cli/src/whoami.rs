use lockbook_core::LbCore;

use crate::error::CliError;

pub fn whoami(core: &LbCore) -> Result<(), CliError> {
    println!("{}", core.get_account()?.username);
    Ok(())
}
