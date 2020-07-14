use lockbook_core::repo::account_repo::{AccountRepo, AccountRepoError};
use lockbook_core::DefaultAccountRepo;

use crate::utils::connect_to_db;

pub fn whoami() {
    match DefaultAccountRepo::get_account(&connect_to_db()) {
        Ok(account) => println!("{}", account.username),
        Err(err) => match err {
            AccountRepoError::SledError(_) | AccountRepoError::SerdeError(_) => {
                eprintln!("Sled error: {:?}", err)
            }
            AccountRepoError::AccountMissing(_) => {
                eprintln!("No account found, run init, import or help.")
            }
        },
    }
}
