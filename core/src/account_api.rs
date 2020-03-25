extern crate reqwest;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::account::Account;
use crate::account_api::Error::{NetworkError, ServerUnavailable, UsernameTaken};
use crate::API_LOC;
use crate::crypto::*;

#[derive(Debug)]
pub enum Error {
    NetworkError(reqwest::Error),
    UsernameTaken,
    ServerUnavailable(u16),
}

pub trait AccountApi {
    fn new_account(account: &Account) -> Result<(), Error>;
}

pub struct AccountApiImpl;

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        NetworkError(e)
    }
}

impl AccountApi for AccountApiImpl {
    fn new_account(account: &Account) -> Result<(), Error> {
        let decrypt_val = format!("{}{}{}",
                                  &account.username,
                                  ",",
                                  SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string());

        let auth = <RsaCryptoService as CryptoService>::encrypt_public( // shouldn't I encrypt private?
            &account.keys.public_key,
            &DecryptedValue { secret: decrypt_val }).unwrap().garbage;

        let params = [
            ("hashed_username", &account.username),
            ("auth", &auth),
            ("pub_key_n", &account.keys.public_key.n),
            ("pub_key_e", &account.keys.public_key.e),
        ];

        let client = reqwest::Client::new();

        let req = client
            .post(format!("{}/new-account", API_LOC).as_str())
            .form(&params)
            .send()?;

        if req.status().is_success() {
            return Ok(());
        }

        match req.status().as_u16() {
            409 => Err(UsernameTaken),
            _ => Err(ServerUnavailable(req.status().as_u16())),
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use std::env;

    use crate::account::Account;
    use crate::account_api::{AccountApi, AccountApiImpl};
    use crate::crypto::{CryptoService, RsaCryptoService};

    type DefaultCrypto = RsaCryptoService;
    type TestAccountApi = AccountApiImpl;

    #[test]
    fn new_account() {
        match env::var("RUN_INTEGRATION_TESTS") {
            Ok(_) => {
                println!("Running integration test: ");
                let username = "parthmehrotra".to_string();
                let keys = DefaultCrypto::generate_key().unwrap();
                let account = Account { username, keys };

                TestAccountApi::new_account(&account).unwrap();
            }
            Err(_) => {
                println!("Env variable RUN_INTEGRATION_TESTS not set, skipping integration test")
            }
        }
    }
}
