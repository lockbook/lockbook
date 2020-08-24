use std::io;
use std::io::Write;

use lockbook_core::{create_account, CreateAccountError};

use crate::utils::{exit_with, get_config};
use crate::{
    ACCOUNT_ALREADY_EXISTS, NETWORK_ISSUE, SUCCESS, UNEXPECTED_ERROR, USERNAME_INVALID,
    USERNAME_TAKEN,
};

pub fn init() {
    print!("Enter a Username: ");
    io::stdout().flush().unwrap();

    let mut username = String::new();
    io::stdin()
        .read_line(&mut username)
        .expect("Failed to read from stdin");

    match create_account(&get_config(), &username) {
        Ok(_) => exit_with("Account created successfully", SUCCESS),
        Err(error) => match error {
            CreateAccountError::UsernameTaken => exit_with("Username taken.", USERNAME_TAKEN),
            CreateAccountError::InvalidUsername => {
                exit_with("Username is not a-z || 0-9", USERNAME_INVALID)
            }
            CreateAccountError::CouldNotReachServer => {
                exit_with("Could not reach server.", NETWORK_ISSUE)
            }
            CreateAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            CreateAccountError::AccountExistsAlready => exit_with(
                "Account already exists. `lockbook erase-everything` to erase your local state.",
                ACCOUNT_ALREADY_EXISTS,
            ),
        },
    }
}
