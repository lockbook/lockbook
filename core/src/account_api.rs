extern crate reqwest;

use std::time::SystemTime;

use crate::account::Account;
use crate::account_api::Error::{NetworkError, ServerUnavailable, UsernameTaken, IncorrectUsername, ExpiredAuth};
use crate::API_LOC;
use crate::crypto::*;
use crate::error_enum;

#[derive(Debug)]
pub enum Error {
    NetworkError(reqwest::Error),
    UsernameTaken,
    IncorrectUsername,
    ServerUnavailable(u16),
    ExpiredAuth,
    DecryptionFailed(DecryptionError)
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
    ) -> Result<(), Error>;
    fn verify_auth_comp(
        auth_time: u128,
        auth_username: &String,
        real_username: &String,
    ) -> Result<(), Error>;
    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<EncryptedValue, EncryptionError>;
}

struct AuthServiceImpl;

impl AuthService for AuthServiceImpl {
    fn verify_auth(
        pub_key: PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), Error> {
        let decrypt_val = RsaCryptoService::decrypt_public(
            &PublicKey {
                n: pub_key.n,
                e: pub_key.e,
            },
            &EncryptedValue {
                garbage: auth.clone(),
            },
        )?;

        let decrypt_comp = decrypt_val.secret.split(",");

        match AuthService::verify_auth_comp(decrypt_comp.next().parse::<u128>()?, username, decrypt_comp.next()) {
            Ok(_) => Ok(()),
            Err(e) => e
        }
    }

    fn verify_auth_comp(
        auth_time: u128,
        auth_username: &String,
        real_username: &String,
    ) -> Result<(), Error> {
        let real_time = SystemTime::now()
            .as_millis();
        let range = real_time - 50..real_time + 50;



        if !range.contains(&auth_time) {
            return Err(ExpiredAuth);
        }
        if real_username != auth_username {
            return Err(IncorrectUsername);
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<EncryptedValue, EncryptionError> {
        let decrypted = format!("{},{}",
                                username,
                                SystemTime::now().as_millis().to_string());

        CryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted })
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        NetworkError(e)
    }
}

impl AccountApi for AccountApiImpl {
    fn new_account(account: &Account) -> Result<(), Error> {
        let auth = AuthService::generate_auth(&account.keys, &account.username)?.garbage;

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
