use std::io;

use lockbook_core::{import_account, Error as CoreError, ImportError};

use crate::utils::{exit_success, get_config};
use crate::{err_extra, err_unexpected, exitlb};

pub fn import_private_key() {
    if atty::is(atty::Stream::Stdin) {
        err_extra!(
            ExpectedStdin,
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import-private-key \
    or xclip -selection clipboard -o | lockbook import-private-key"
        )
        .exit();
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
                CoreError::UiError(err) => match err {
                    ImportError::AccountStringCorrupted => exitlb!(AccountStringCorrupted),
                    ImportError::AccountExistsAlready => exitlb!(AccountAlreadyExists),
                    ImportError::AccountDoesNotExist => exitlb!(AccountDoesNotExist),
                    ImportError::UsernamePKMismatch => exitlb!(UsernamePkMismatch),
                    ImportError::CouldNotReachServer => exitlb!(NetworkIssue),
                    ImportError::ClientUpdateRequired => exitlb!(UpdateRequired),
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            },
        }
    }
}
