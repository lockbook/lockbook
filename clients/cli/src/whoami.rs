use crate::error::CliResult;
use crate::utils::get_account_or_exit;

pub fn whoami() -> CliResult {
    println!("{}", get_account_or_exit().username);
    Ok(())
}
