use std::io;

use lockbook_core::service::account_service::{AccountImportError, AccountService};

use lockbook_core::DefaultAccountService;

use crate::utils::connect_to_db;

pub fn import() {
    let db = connect_to_db();

    println!(
        "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import \
    or xclip -selection clipboard -o | lockbook import."
    );

    let mut account_string = String::new();
    io::stdin()
        .read_line(&mut account_string)
        .expect("Failed to read from stdin");

    println!("Importing...");

    match DefaultAccountService::import_account(&db, &account_string) {
        Ok(_) => println!("Account imported successfully!"),
        Err(err) => match err {
            AccountImportError::AccountStringCorrupted(_) => eprintln!("Account String corrupted!"),
            AccountImportError::PersistenceError(_) => eprintln!("Could not persist data!"),
            AccountImportError::InvalidPrivateKey(_) => eprintln!("Invalid private key!"),
            AccountImportError::AccountStringFailedToDeserialize(_) => {
                eprintln!("Account String corrupted!")
            }
        },
    }
}
