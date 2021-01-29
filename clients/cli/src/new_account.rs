use std::io::Write;
use std::{env, io};

use lockbook_core::{create_account, CreateAccountError, Error as CoreError};

use crate::utils::{exit_success, get_config};
use crate::{err_unexpected, exitlb};

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
            CoreError::UiError(err) => match err {
                CreateAccountError::UsernameTaken => exitlb!(UsernameTaken(username)),
                CreateAccountError::InvalidUsername => exitlb!(UsernameInvalid(username)),
                CreateAccountError::AccountExistsAlready => exitlb!(AccountAlreadyExists),
                CreateAccountError::CouldNotReachServer => exitlb!(NetworkIssue),
                CreateAccountError::ClientUpdateRequired => exitlb!(UpdateRequired),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
