use std::io;
use std::process::exit;

use lockbook_core::service::account_service::{AccountImportError, AccountService};
use lockbook_core::DefaultAccountService;

use crate::utils::{connect_to_db, exit_with};
use crate::EXPECTED_STDIN;

pub fn import() {
    if atty::is(atty::Stream::Stdin) {
        exit_with(
            "To import an existing Lockbook, pipe your account string into this command, \
    eg. pbpaste | lockbook import \
    or xclip -selection clipboard -o | lockbook import",
            EXPECTED_STDIN,
        );
    } else {
        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("Failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("Importing...");

        match DefaultAccountService::import_account(&connect_to_db(), &account_string) {
            Ok(_) => println!("Account imported successfully!"),
            Err(err) => match err {
                AccountImportError::AccountStringCorrupted(_) => {
                    eprintln!("Account String corrupted!")
                }
                AccountImportError::PersistenceError(_) => eprintln!("Could not persist data!"),
                AccountImportError::InvalidPrivateKey(_) => eprintln!("Invalid private key!"),
                AccountImportError::AccountStringFailedToDeserialize(_) => {
                    eprintln!("Account String corrupted!")
                }
            },
        }
    }
}
