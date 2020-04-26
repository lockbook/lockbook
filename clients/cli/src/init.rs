use std::io::Write;

use std::io;

use lockbook_core::client::NewAccountError;

use lockbook_core::service::account_service::{AccountCreationError, AccountService};

use lockbook_core::DefaultAccountService;

use crate::connect_to_db;

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

            AccountCreationError::PersistenceError(_) => {
                eprintln!("Could not persist data, error: ")
            }

            AccountCreationError::ApiError(api_err) => match api_err {
                NewAccountError::SendFailed(_) => eprintln!("Network Error Occurred"),
                NewAccountError::UsernameTaken => {
                    eprintln!("Username {} not available!", &username)
                }
                _ => eprintln!("Unknown Error Occurred!"),
            },

            AccountCreationError::AuthGenFailure(_) => {
                eprintln!("Could not use private key to sign message")
            }

            AccountCreationError::KeySerializationError(_) => eprintln!("Could not serialize key"),
        },
    }
}
