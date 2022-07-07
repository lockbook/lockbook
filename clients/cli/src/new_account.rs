use std::io::Write;
use std::{env, io};

use lockbook_core::Core;
use lockbook_core::CreateAccountError;
use lockbook_core::Error as LbError;

use crate::error::CliError;

pub fn new_account(core: &Core) -> Result<(), CliError> {
    print!("Enter a Username: ");
    io::stdout().flush()?;

    let mut username = String::new();
    io::stdin()
        .read_line(&mut username)
        .expect("Failed to read from stdin");
    username.retain(|c| c != '\n' && c != '\r');

    let api_location =
        env::var("API_URL").unwrap_or_else(|_| lockbook_core::DEFAULT_API_LOCATION.to_string());

    println!("Generating keys and checking for username availability...");

    core.create_account(&username, &api_location)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                CreateAccountError::UsernameTaken => CliError::username_taken(&username),
                CreateAccountError::InvalidUsername => CliError::username_invalid(&username),
                CreateAccountError::AccountExistsAlready => CliError::account_exists(),
                CreateAccountError::CouldNotReachServer => CliError::network_issue(),
                CreateAccountError::ClientUpdateRequired => CliError::update_required(),
                CreateAccountError::ServerDisabled => CliError::server_disabled(),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    println!("Account created successfully.");
    Ok(())
}
