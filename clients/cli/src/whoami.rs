use lockbook_core::{get_account, GetAccountError};

use crate::utils::{exit_with, exit_with_no_account, get_config};
use crate::UNEXPECTED_ERROR;

pub fn whoami() {
    match get_account(&get_config()) {
        Ok(account) => println!("{}", account.username),
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
