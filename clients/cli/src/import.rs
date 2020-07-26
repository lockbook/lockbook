use std::io;

use lockbook_core::{import_account, ImportError};

use crate::utils::{exit_with, get_config};
use crate::{ACCOUNT_STRING_CORRUPTED, EXPECTED_STDIN, SUCCESS, UNEXPECTED_ERROR};

pub fn import() {
    if atty::is(atty::Stream::Stdin) {
        exit_with(
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import \
    or xclip -selection clipboard -o | lockbook import",
            EXPECTED_STDIN,
        );
    } else {
        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("Failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("Importing...");

        match import_account(&get_config(), &account_string) {
            Ok(_) => exit_with("Account imported successfully", SUCCESS),
            Err(err) => match err {
                ImportError::AccountStringCorrupted => exit_with(
                    "Account string corrupted, not imported",
                    ACCOUNT_STRING_CORRUPTED,
                ),
                ImportError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        }
    }
}
