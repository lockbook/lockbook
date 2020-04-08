extern crate reqwest;

use std::num::ParseIntError;
use std::option::NoneError;
use std::time::SystemTime;
use std::time::SystemTimeError;

use crate::account::Account;
use crate::account_api::Error::{ExpiredAuth, IncorrectUsername, NetworkError, ServerUnavailable, UsernameTaken, CryptoError};
use crate::API_LOC;
use crate::crypto::*;
use crate::error_enum;

#[derive(Debug)]
pub enum Error {
    NetworkError(reqwest::Error),
    UsernameTaken,
    IncorrectUsername,
    ExpiredAuth,
    CryptoError,
    ServerUnavailable(u16),
}

error_enum! {
    enum AuthError {
        DecryptionFailure(DecryptionError),
        ParseError(ParseIntError),
        IncompleteAuth(NoneError),
        NegativeTime(SystemTimeError),
        AuthGenFailed(EncryptionError),
        IncorrectAuth(Error)
    }
}

pub trait AccountApi {
    fn new_account(account: &Account) -> Result<(), Error>;
}

pub struct AccountApiImpl;

pub trait AuthService {
    fn verify_auth(
        pub_key: PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), AuthError>;
    fn verify_auth_comp(
        auth_time: &u128,
        auth_username: &String,
        real_username: &String,
    ) -> Result<(), AuthError>;
    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<EncryptedValue, AuthError>;
}

struct AuthServiceImpl;

impl AuthService for AuthServiceImpl {
    fn verify_auth(
        pub_key: PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), AuthError> {
        let decrypt_val = RsaCryptoService::decrypt_public(
            &PublicKey {
                n: pub_key.n,
                e: pub_key.e,
            },
            &EncryptedValue {
                garbage: auth.clone(),
            },
        )?;

        let mut auth_comp = decrypt_val.secret.split(",");

        match AuthServiceImpl::verify_auth_comp(
            &auth_comp.next()?.parse::<u128>()?,
            &username,
            &String::from(auth_comp.next()?)) {
            Ok(_) => Ok(()),
            Err(e) => Err(e)
        }
    }

    fn verify_auth_comp(
        auth_time: &u128,
        auth_username: &String,
        real_username: &String,
    ) -> Result<(), AuthError> {
        let real_time = SystemTime::now()
            .elapsed()?.as_millis();

        if real_username != auth_username {
            return Err(AuthError::IncorrectAuth(IncorrectUsername));
        }
        let range = real_time..real_time + 50;
        if !range.contains(&auth_time) {
            return Err(AuthError::IncorrectAuth(ExpiredAuth));
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<EncryptedValue, AuthError> {
        let decrypted = format!("{},{}",
                                username,
                                SystemTime::now().elapsed()?.as_millis().to_string());

        match RsaCryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted }) {
            Ok(i) => Ok(i),
            Err(e) => Err(AuthError::AuthGenFailed(e))
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        NetworkError(e)
    }
}

impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::IncorrectAuth(IncorrectUsername) => IncorrectUsername,
            AuthError::IncorrectAuth(ExpiredAuth) => ExpiredAuth,
            _ => CryptoError
        }
    }
}

impl AccountApi for AccountApiImpl {
    fn new_account(account: &Account) -> Result<(), Error> {
        let auth = AuthServiceImpl::generate_auth(&account.keys, &account.username)?.garbage;

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
