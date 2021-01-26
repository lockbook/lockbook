use std::io::Write;
use std::{env, io};

use lockbook_core::{create_account, CreateAccountError, Error as CoreError};

use crate::exitlb;
use crate::utils::{exit_success, exit_with_offline, exit_with_upgrade_required, get_config};

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
        Ok(_) => exit_success("Account created successfully"),
        Err(error) => match error {
            CoreError::UiError(CreateAccountError::UsernameTaken) => {
                exitlb!(UsernameTaken, "Username taken.")
            }
            CoreError::UiError(CreateAccountError::InvalidUsername) => {
                exitlb!(UsernameInvalid, "Username is not a-z || 0-9")
            }
            CoreError::UiError(CreateAccountError::CouldNotReachServer) => exit_with_offline(),
            CoreError::UiError(CreateAccountError::AccountExistsAlready) => exitlb!(
                AccountAlreadyExists,
                "Account already exists. `lockbook erase-everything` to erase your local state."
            ),
            CoreError::UiError(CreateAccountError::ClientUpdateRequired) => {
                exit_with_upgrade_required()
            }
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    }
}
