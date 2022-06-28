use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::ImportError;

use crate::error::CliError;

pub fn import_private_key(core: &Core) -> Result<(), CliError> {
    if atty::is(atty::Stream::Stdin) {
        Err(CliError::expected_stdin().with_extra(
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import-private-key \
    or xclip -selection clipboard -o | lockbook import-private-key",
        ))
    } else {
        let mut account_string = String::new();
        std::io::stdin()
            .read_line(&mut account_string)
            .expect("Failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("Importing...");

        core.import_account(&account_string)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    ImportError::AccountStringCorrupted => CliError::account_string_corrupted(),
                    ImportError::AccountExistsAlready => CliError::account_exists(),
                    ImportError::AccountDoesNotExist => CliError::account_not_on_server(),
                    ImportError::UsernamePKMismatch => CliError::username_pk_mismatch(),
                    ImportError::CouldNotReachServer => CliError::network_issue(),
                    ImportError::ClientUpdateRequired => CliError::update_required(),
                },
                LbError::Unexpected(msg) => CliError::unexpected(msg),
            })?;

        println!("Account imported successfully.");
        Ok(())
    }
}
