use std::io::Write;
use std::{env, io};

use lockbook_core::{create_account, CreateAccountError, Error as CoreError};

use crate::error::CliResult;
use crate::utils::{config, grab_line_stdin};
use crate::{err, err_unexpected};

pub fn new_account() -> CliResult<()> {
    let username = grab_line_stdin("Enter a Username: ")?;

    let api_location =
        env::var("API_URL").unwrap_or_else(|_| lockbook_core::DEFAULT_API_LOCATION.to_string());

    println!("Generating keys and checking for username availability...");

    create_account(&config()?, &username, &api_location).map_err(|err| match err {
        CoreError::UiError(err) => match err {
            CreateAccountError::UsernameTaken => err!(UsernameTaken(username)),
            CreateAccountError::InvalidUsername => err!(UsernameInvalid(username)),
            CreateAccountError::AccountExistsAlready => err!(AccountAlreadyExists),
            CreateAccountError::CouldNotReachServer => err!(NetworkIssue),
            CreateAccountError::ClientUpdateRequired => err!(UpdateRequired),
        },
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    println!("Account created successfully.");
    Ok(())
}
