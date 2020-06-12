use std::num::ParseIntError;

use rsa::RSAPublicKey;
use serde::export::PhantomData;

use crate::error_enum;
use crate::model::account::Account;
use crate::model::crypto::*;
use crate::service::auth_service::VerificationError::{
    AuthDeserializationError, CryptoVerificationError, InvalidAuthLayout, InvalidUsername,
    TimeStampOutOfBounds, TimeStampParseFailure,
};
use crate::service::clock_service::Clock;
use crate::service::crypto_service::{PubKeyCryptoService, SignatureVerificationFailed};

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    CryptoVerificationError(SignatureVerificationFailed),
    InvalidAuthLayout(()),
    AuthDeserializationError(serde_json::error::Error),
    InvalidUsername,
    TimeStampOutOfBounds(u128),
}

impl From<ParseIntError> for VerificationError {
    fn from(e: ParseIntError) -> Self {
        TimeStampParseFailure(e)
    }
}

impl From<SignatureVerificationFailed> for VerificationError {
    fn from(e: SignatureVerificationFailed) -> Self {
        CryptoVerificationError(e)
    }
}

impl From<()> for VerificationError {
    fn from(_e: ()) -> Self {
        InvalidAuthLayout(())
    }
}

impl From<serde_json::error::Error> for VerificationError {
    fn from(e: serde_json::error::Error) -> Self {
        AuthDeserializationError(e)
    }
}

error_enum! {
    enum AuthGenError {
        RsaError(rsa::errors::Error),
        AuthSerializationError(serde_json::error::Error)
    }
}

pub trait AuthService {
    fn verify_auth(
        auth: &str,
        public_key: &RSAPublicKey,
        username: &str,
        max_auth_delay: u128,
    ) -> Result<(), VerificationError>;
    fn generate_auth(account: &Account) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl<Time: Clock, Crypto: PubKeyCryptoService> {
    clock: PhantomData<Time>,
    crypto: PhantomData<Crypto>,
}

impl<Time: Clock, Crypto: PubKeyCryptoService> AuthService for AuthServiceImpl<Time, Crypto> {
    fn verify_auth(
        auth: &str,
        public_key: &RSAPublicKey,
        username: &str,
        max_auth_delay: u128,
    ) -> Result<(), VerificationError> {
        let signed_val = serde_json::from_str::<SignedValue>(&String::from(auth))?;
        Crypto::verify(&public_key, &signed_val)?;

        let mut auth_comp = signed_val.content.split(',');

        if &String::from(auth_comp.next().ok_or(())?) != username {
            return Err(InvalidUsername);
        }

        let auth_time = auth_comp.next().ok_or(())?.parse::<u128>()?;
        let range = auth_time..auth_time + max_auth_delay;
        let current_time = Time::get_time();

        if !range.contains(&current_time) {
            return Err(TimeStampOutOfBounds(current_time - auth_time));
        }
        Ok(())
    }

    fn generate_auth(account: &Account) -> Result<String, AuthGenError> {
        let to_sign = format!("{},{}", &account.username, Time::get_time().to_string());

        Ok(serde_json::to_string(&Crypto::sign(
            &account.keys,
            &to_sign,
        )?)?)
    }
}

#[cfg(test)]
mod unit_tests {
    use std::mem::discriminant;

    use rand::rngs::OsRng;
    use rsa::RSAPrivateKey;

    use crate::model::account::Account;
    use crate::service::auth_service::{AuthService, AuthServiceImpl, VerificationError};
    use crate::service::clock_service::Clock;
    use crate::service::crypto_service::RsaImpl;

    struct EarlyClock;

    impl Clock for EarlyClock {
        fn get_time() -> u128 {
            500
        }
    }

    struct LateClock;

    impl Clock for LateClock {
        fn get_time() -> u128 {
            520
        }
    }

    #[test]
    fn test_auth_inverse_property() {
        let private_key = RSAPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key = private_key.to_public_key();

        let username = String::from("Smail");

        let account = Account {
            username: username.clone(),
            keys: private_key,
        };
        let auth = AuthServiceImpl::<EarlyClock, RsaImpl>::generate_auth(&account).unwrap();
        AuthServiceImpl::<LateClock, RsaImpl>::verify_auth(&auth, &public_key, &username, 100)
            .unwrap()
    }

    #[test]
    fn test_auth_invalid_username() {
        let private_key = RSAPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key = private_key.to_public_key();

        let username = String::from("Smail");
        let account = Account {
            username,
            keys: private_key,
        };

        let auth = AuthServiceImpl::<EarlyClock, RsaImpl>::generate_auth(&account).unwrap();

        let result = discriminant(
            &AuthServiceImpl::<LateClock, RsaImpl>::verify_auth(
                &auth,
                &public_key,
                &String::from("Hamza"),
                100,
            )
            .unwrap_err(),
        );
        let error = discriminant(&VerificationError::InvalidUsername);

        assert_eq!(result, error);
    }
}
