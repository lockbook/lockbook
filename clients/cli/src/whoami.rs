use crate::error::CliResult;
use crate::utils::account;

pub fn whoami() -> CliResult<()> {
    println!("{}", account()?.username);
    Ok(())
}
