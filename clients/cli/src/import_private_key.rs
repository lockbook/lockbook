use std::io;

use lockbook_core::{import_account, ImportError};

use crate::utils::{exit_with, exit_with_offline, exit_with_upgrade_required, get_config};
use crate::{
    ACCOUNT_ALREADY_EXISTS, ACCOUNT_DOES_NOT_EXIST, ACCOUNT_STRING_CORRUPTED, EXPECTED_STDIN,
    SUCCESS, UNEXPECTED_ERROR, USERNAME_PK_MISMATCH,
};

pub fn import_private_key() {
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
                ImportError::AccountExistsAlready => exit_with("Account already exists. `lockbook erase-everything` to erase your local state.", ACCOUNT_ALREADY_EXISTS),
                ImportError::AccountDoesNotExist => exit_with("An account with this username was not found on the server.", ACCOUNT_DOES_NOT_EXIST),
                ImportError::UsernamePKMismatch => exit_with("The public_key in this account_string does not match what is on the server", USERNAME_PK_MISMATCH),
                ImportError::CouldNotReachServer => exit_with_offline(),
                ImportError::ClientUpdateRequired => exit_with_upgrade_required(),
            },
        }
    }
}
