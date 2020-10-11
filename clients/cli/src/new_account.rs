use std::io::Write;
use std::{env, io};

use lockbook_core::{create_account, CreateAccountError, Error as CoreError};

use crate::utils::{exit_with, exit_with_offline, exit_with_upgrade_required, get_config};
use crate::{ACCOUNT_ALREADY_EXISTS, SUCCESS, UNEXPECTED_ERROR, USERNAME_INVALID, USERNAME_TAKEN};

pub fn new_account() {
    print!("Enter a Username: ");
    io::stdout().flush().unwrap();

    let mut username = String::new();
    io::stdin()
        .read_line(&mut username)
        .expect("Failed to read from stdin");
    username.retain(|c| c != '\n');

    let api_location =
        env::var("API_URL").unwrap_or_else(|_| lockbook_core::DEFAULT_API_LOCATION.to_string());

    println!("Generating keys and checking for username availability...");

    match create_account(&get_config(), &username, &api_location) {
        Ok(_) => exit_with("Account created successfully", SUCCESS),
        Err(error) => match error {
            CoreError::UiError(CreateAccountError::UsernameTaken) => {
                exit_with("Username taken.", USERNAME_TAKEN)
            }
            CoreError::UiError(CreateAccountError::InvalidUsername) => {
                exit_with("Username is not a-z || 0-9", USERNAME_INVALID)
            }
            CoreError::UiError(CreateAccountError::CouldNotReachServer) => exit_with_offline(),
            CoreError::UiError(CreateAccountError::AccountExistsAlready) => exit_with(
                "Account already exists. `lockbook erase-everything` to erase your local state.",
                ACCOUNT_ALREADY_EXISTS,
            ),
            CoreError::UiError(CreateAccountError::ClientUpdateRequired) => {
                exit_with_upgrade_required()
            }
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
