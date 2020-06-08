use lockbook_core::repo::account_repo::Error;
use lockbook_core::service::account_service::{AccountExportError, AccountService};

use lockbook_core::DefaultAccountService;

use crate::utils::connect_to_db;

pub fn export() {
    let db = connect_to_db();

    match DefaultAccountService::export_account(&db) {
        Ok(account_string) => {
            match qr2term::print_qr(&account_string) {
                Ok(_) => {}
                Err(qr_err) => eprintln!(
                    "Unexpected error occured while generating qr code: {:?}",
                    qr_err
                ),
            }
            println!("For the raw string, copy the next line (triple click):");
            println!("{}", account_string);
        }
        Err(err) => match &err {
            AccountExportError::KeyRetrievalError(db_err) => match db_err {
                Error::AccountMissing(_) => {
                    eprintln!("No account found, run init, import or help.")
                }
                _ => eprintln!("Unexpected error occurred: {:?}", err),
            },
            _ => eprintln!("Unexpected error occurred: {:?}", err),
        },
    }
}
