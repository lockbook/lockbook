use std::io::Write;
use std::{env, io};

use lockbook_core::model::errors::CreateAccountError;
use lockbook_core::{Error as CoreError, LbCore};

use crate::error::CliResult;
use crate::{err, err_unexpected};

pub fn new_account(core: &LbCore) -> CliResult<()> {
    print!("Enter a Username: ");
    io::stdout().flush().unwrap();

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
            CoreError::UiError(err) => match err {
                CreateAccountError::UsernameTaken => err!(UsernameTaken(username)),
                CreateAccountError::InvalidUsername => err!(UsernameInvalid(username)),
                CreateAccountError::AccountExistsAlready => err!(AccountAlreadyExists),
                CreateAccountError::CouldNotReachServer => err!(NetworkIssue),
                CreateAccountError::ClientUpdateRequired => err!(UpdateRequired),
                CreateAccountError::ServerDisabled => err!(ServerDisabled),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })?;

    println!("Account created successfully.");
    Ok(())
}
