use lockbook_core::model::errors::AccountExportError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;

pub fn export_private_key(core: &Core) -> Result<(), CliError> {
    let account_string = core.export_account().map_err(|err| match err {
        LbError::UiError(AccountExportError::NoAccount) => CliError::no_account(),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    if atty::is(atty::Stream::Stdout) {
        qr2term::print_qr(&account_string)
            .map_err(|qr_err| CliError::unexpected(format!("generating qr code: {}", qr_err)))?;
    } else {
        println!("{}", account_string);
    }

    Ok(())
}
