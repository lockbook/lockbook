use lockbook_core::repo::account_repo::{AccountRepo, Error};
use lockbook_core::DefaultAccountRepo;

use crate::utils::connect_to_db;

pub fn whoami() {
    match DefaultAccountRepo::get_account(&connect_to_db()) {
        Ok(account) => println!("{}", account.username),
        Err(err) => match err {
            Error::SledError(_) | Error::SerdeError(_) => eprintln!("Sled error: {:?}", err),
            Error::AccountMissing(_) => eprintln!("No account found, run init, import or help."),
        },
    }
}
