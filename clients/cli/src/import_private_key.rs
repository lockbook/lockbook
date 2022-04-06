use crate::error::CliResult;
use crate::utils::config;
use crate::{err, err_extra, err_unexpected};
use lockbook_core::model::errors::ImportError;
use lockbook_core::{import_account, Error as CoreError};

pub fn import_private_key() -> CliResult<()> {
    if atty::is(atty::Stream::Stdin) {
        Err(err_extra!(
            ExpectedStdin,
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import-private-key \
    or xclip -selection clipboard -o | lockbook import-private-key"
        ))
    } else {
        let mut account_string = String::new();
        std::io::stdin()
            .read_line(&mut account_string)
            .expect("Failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("Importing...");

        import_account(&config()?, &account_string).map_err(|err| match err {
            CoreError::UiError(err) => match err {
                ImportError::AccountStringCorrupted => err!(AccountStringCorrupted),
                ImportError::AccountExistsAlready => err!(AccountAlreadyExists),
                ImportError::AccountDoesNotExist => err!(AccountDoesNotExistOnServer),
                ImportError::UsernamePKMismatch => err!(UsernamePkMismatch),
                ImportError::CouldNotReachServer => err!(NetworkIssue),
                ImportError::ClientUpdateRequired => err!(UpdateRequired),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })?;

        println!("Account imported successfully.");
        Ok(())
    }
}
