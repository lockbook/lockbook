use lockbook_core::{export_account, AccountExportError, Error as CoreError};

use crate::utils::get_config;
use crate::{err_unexpected, exitlb};

pub fn export_private_key() {
    match export_account(&get_config()) {
        Ok(account_string) => {
            if atty::is(atty::Stream::Stdout) {
                match qr2term::print_qr(&account_string) {
                    Ok(_) => {}
                    Err(qr_err) => eprintln!(
                        "Unexpected error occured while generating qr code: {:?}",
                        qr_err
                    ),
                }
            } else {
                println!("{}", account_string);
            }
        }
        Err(err) => match err {
            CoreError::UiError(AccountExportError::NoAccount) => exitlb!(NoAccount),
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
