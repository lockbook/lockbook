use crate::account::Account;

error_enum! {
    enum Error {

    }
}

pub trait AcountApi {
    fn new_account(account: &Account) -> Result<(), Error>;
}
