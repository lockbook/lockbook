use std::io;
use std::io::Write;

use lockbook_core::client::Error;
use lockbook_core::model::api::NewAccountError;
use lockbook_core::service::account_service::{AccountCreationError, AccountService};
use lockbook_core::DefaultAccountService;

use crate::utils::connect_to_db;

pub fn init() {
    let db = connect_to_db();

    print!("Enter a Username: ");
    io::stdout().flush().unwrap();

    let mut username = String::new();
    io::stdin()
        .read_line(&mut username)
        .expect("Failed to read from stdin");
    username.retain(|c| !c.is_whitespace());

    match DefaultAccountService::create_account(&db, &username) {
        Ok(_) => println!("Account created successfully!"),
        Err(err) => match err {
            AccountCreationError::KeyGenerationError(e) => {
                eprintln!("Could not generate keypair, error: {}", e)
            }

            AccountCreationError::PersistenceError(err) => {
                eprintln!("Could not persist data, error: {:?}", err)
            }

            AccountCreationError::ApiError(api_err) => match api_err {
                Error::SendFailed(_) => eprintln!("Network error: {:?}", api_err),
                Error::Api(api_err_err) => match api_err_err {
                    NewAccountError::UsernameTaken => eprintln!("Username Taken!"),
                    _ => eprintln!("Unexpected error occurred: {:?}", api_err_err),
                },
                _ => eprintln!("Unexpected error occurred: {:?}", api_err),
            },

            _ => eprintln!("Unexpected error occurred: {:?}", err),
        },
    }
}
