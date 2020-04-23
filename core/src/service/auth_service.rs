use std::num::ParseIntError;
use std::option::NoneError;

use rsa::{RSAPrivateKey, RSAPublicKey};
use serde::export::PhantomData;

use crate::service::auth_service::VerificationError::{
    AuthDeserializationError, CryptoVerificationError, InvalidAuthLayout, InvalidUsername,
    TimeStampOutOfBounds, TimeStampParseFailure,
};
use crate::service::crypto_service::{SignatureVerificationFailed, PubKeyCryptoService, SignedValue};
use crate::service::clock_service::Clock;
use crate::error_enum;

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    CryptoVerificationError(SignatureVerificationFailed),
    InvalidAuthLayout(NoneError),
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

impl From<NoneError> for VerificationError {
    fn from(e: NoneError) -> Self {
        InvalidAuthLayout(e)
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
        auth: &String,
        public_key: &RSAPublicKey,
        username: &String,
        max_auth_delay: u128,
    ) -> Result<(), VerificationError>;
    fn generate_auth(
        private_key: &RSAPrivateKey,
        username: &String,
    ) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl<Time: Clock, Crypto: PubKeyCryptoService> {
    clock: PhantomData<Time>,
    crypto: PhantomData<Crypto>,
}

impl<Time: Clock, Crypto: PubKeyCryptoService> AuthService for AuthServiceImpl<Time, Crypto> {
    fn verify_auth(
        auth: &String,
        public_key: &RSAPublicKey,
        username: &String,
        max_auth_delay: u128,
    ) -> Result<(), VerificationError> {
        let signed_val = serde_json::from_str::<SignedValue>(&String::from(auth))?;
        Crypto::verify(&public_key, &signed_val)?;

        let mut auth_comp = signed_val.content.split(",");

        if &String::from(auth_comp.next()?) != username {
            return Err(InvalidUsername);
        }

        let auth_time = auth_comp.next()?.parse::<u128>()?;
        let range = auth_time..auth_time + max_auth_delay;
        let current_time = Time::get_time();

        if !range.contains(&current_time) {
            return Err(TimeStampOutOfBounds(current_time - auth_time));
        }
        Ok(())
    }

    fn generate_auth(
        private_key: &RSAPrivateKey,
        username: &String,
    ) -> Result<String, AuthGenError> {
        let to_sign = format!("{},{}", username, Time::get_time().to_string());

        Ok(serde_json::to_string(&Crypto::sign(
            &private_key,
            to_sign,
        )?)?)
    }
}

#[cfg(test)]
mod unit_tests {
    use std::mem::discriminant;

    use rand::rngs::OsRng;
    use rsa::{RSAPrivateKey, RSAPublicKey};

    use crate::service::crypto_service::{RsaImpl, SignedValue};
    use crate::service::auth_service::{VerificationError, AuthServiceImpl, AuthService};
    use crate::service::clock_service::Clock;

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
        let auth =
            AuthServiceImpl::<EarlyClock, RsaImpl>::generate_auth(&private_key, &username)
                .unwrap();
        AuthServiceImpl::<LateClock, RsaImpl>::verify_auth(
            &auth,
            &public_key,
            &username,
            100
        )
            .unwrap()
    }

    #[test]
    fn test_auth_invalid_username() {
        let private_key = RSAPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key = private_key.to_public_key();

        let username = String::from("Smail");
        let auth =
            AuthServiceImpl::<EarlyClock, RsaImpl>::generate_auth(&private_key, &username)
                .unwrap();

        let result = discriminant(
            &AuthServiceImpl::<LateClock, RsaImpl>::verify_auth(
                &auth,
                &public_key,
                &String::from("Hamza"),
                100
            )
                .unwrap_err(),
        );
        let error = discriminant(&VerificationError::InvalidUsername);

        assert_eq!(result, error);
    }
}
