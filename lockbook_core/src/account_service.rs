use std::marker::PhantomData;

use crate::account_repo::AccountRepo;
use crate::crypto::CryptoService;

pub trait AccountService {
    fn create_account(username: String);
}

pub struct AccountServiceImpl<Crypto: CryptoService, Accounts: AccountRepo> {
    encyption: PhantomData<Crypto>,
    acounts: PhantomData<Accounts>,
}

impl<Crypto: CryptoService, Accounts: AccountRepo> AccountService for AccountServiceImpl<Crypto, Accounts> {
    fn create_account(username: String) {
        Crypto::generate_key();
    }
}