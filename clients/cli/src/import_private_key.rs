use std::io;

use lockbook_core::{import_account, Error as CoreError, ImportError};

use crate::exitlb;
use crate::utils::{exit_success, exit_with_offline, exit_with_upgrade_required, get_config};

pub fn import_private_key() {
    if atty::is(atty::Stream::Stdin) {
        exitlb!(
            ExpectedStdin,
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import-private-key \
    or xclip -selection clipboard -o | lockbook import-private-key"
        );
    } else {
        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("Failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("Importing...");

        match import_account(&get_config(), &account_string) {
            Ok(_) => exit_success("Account imported successfully"),
            Err(err) => match err {
                CoreError::UiError(ImportError::AccountStringCorrupted) => exitlb!(
                    AccountStringCorrupted,
                    "Account string corrupted, not imported"
                ),
                CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
                CoreError::UiError(ImportError::AccountExistsAlready) => exitlb!(AccountAlreadyExists, "Account already exists. `lockbook erase-everything` to erase your local state."),
                CoreError::UiError(ImportError::AccountDoesNotExist) => exitlb!(AccountDoesNotExist, "An account with this username was not found on the server."),
                CoreError::UiError(ImportError::UsernamePKMismatch) => exitlb!(UsernamePkMismatch, "The public_key in this account_string does not match what is on the server"),
                CoreError::UiError(ImportError::CouldNotReachServer) => exit_with_offline(),
                CoreError::UiError(ImportError::ClientUpdateRequired) => exit_with_upgrade_required(),
            },
        }
    }
}
