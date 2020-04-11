extern crate reqwest;

use crate::account::Account;
use crate::account_api::Error::{NetworkError, ServerUnavailable, UsernameTaken};
use crate::API_LOC;
use crate::auth_service::{AuthServiceImpl, AuthService};

#[derive(Debug)]
pub enum Error {
    NetworkError(reqwest::Error),
    UsernameTaken,
    CryptoError,
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
        let auth = AuthServiceImpl::generate_auth(&account.keys, &account.username)?;

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
    use crate::crypto::{CryptoService, RsaCryptoService, DecryptedValue};
    use crate::auth_service::{AuthServiceImpl, AuthService};

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

    #[test]
    fn test_auth_time_in_bounds() {
        let keys = DefaultCrypto::generate_key().unwrap();
        let username = String::from("Smail");
        let auth = AuthServiceImpl::generate_auth(&keys, &username).unwrap();
        AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap();
    }

    #[test]
    fn test_auth_time_expired() {
        let keys = DefaultCrypto::generate_key().unwrap();
        let username = String::from("Smail");

        let decrypt_auth = format!("{},{}", username, 3);
        let auth = RsaCryptoService::encrypt_private(
            &keys,
            &DecryptedValue { secret: decrypt_auth }).unwrap().garbage;

        let result = AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth);

        match result {
            Ok(()) => panic!("Verifying auth passed when it shouldn't have!"),
            Err(_) => ()
        }
    }
}
