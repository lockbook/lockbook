use lockbook_core::{export_account, AccountExportError, Error as CoreError};

use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};

pub fn export_private_key() -> CliResult {
    let account_string = export_account(&get_config()).map_err(|err| match err {
        CoreError::UiError(AccountExportError::NoAccount) => err!(NoAccount),
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    if atty::is(atty::Stream::Stdout) {
        if let Err(qr_err) = qr2term::print_qr(&account_string) {
            return Err(err_unexpected!("generating qr code: {}", qr_err));
        }
    } else {
        println!("{}", account_string);
    }

    Ok(())
}
