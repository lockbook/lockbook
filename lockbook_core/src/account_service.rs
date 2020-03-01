use crate::encryption;
use std::marker::PhantomData;
use crate::encryption::Error;
use openssl::pkey::Private;

pub trait AccountService {
    fn create_account(username: String);
}

pub struct AccountServiceImpl<Encryption: encryption::Encryption> {
    encyption: PhantomData<Encryption>,
}

impl <Encryption: encryption::Encryption> AccountService for AccountServiceImpl<Encryption> {
    fn create_account(username: String) {
        Encryption::generate_key();
    }
}