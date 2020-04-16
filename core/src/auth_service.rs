use crate::auth_service::VerificationError::{
    AuthDeserializationError, CryptoVerificationError, InvalidAuthLayout, InvalidTimeStamp,
    InvalidUsername, TimeStampOutOfBounds, TimeStampParseFailure,
};
use crate::clock::Clock;
use crate::crypto::{PubKeyCryptoService, SignatureVerificationFailed, SignedValue};
use crate::error_enum;

use rsa::{PublicKey, RSAPrivateKey, RSAPublicKey};

use crate::model::state::Config;
use serde::de::IntoDeserializer;
use serde::export::PhantomData;
use std::num::ParseIntError;
use std::option::NoneError;
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    CryptoVerificationError(SignatureVerificationFailed),
    InvalidAuthLayout(NoneError),
    InvalidTimeStamp(SystemTimeError),
    AuthDeserializationError(serde_json::error::Error),
    InvalidUsername,
    TimeStampOutOfBounds,
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

impl From<SystemTimeError> for VerificationError {
    fn from(e: SystemTimeError) -> Self {
        InvalidTimeStamp(e)
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
    ) -> Result<(), VerificationError>;
    fn generate_auth(
        private_key: &RSAPrivateKey,
        username: &String,
    ) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl<Time: Clock, Crypto: PubKeyCryptoService> {
    //better name
    clock: PhantomData<Time>,
    crypto: PhantomData<Crypto>,
}

impl<Time: Clock, Crypto: PubKeyCryptoService> AuthService for AuthServiceImpl<Time, Crypto> {
    fn verify_auth(
        auth: &String,
        public_key: &RSAPublicKey,
        username: &String,
    ) -> Result<(), VerificationError> {
        let signed_val = serde_json::from_str::<SignedValue>(&String::from(auth))?;
        Crypto::verify(&public_key, &signed_val)?;

        let mut auth_comp = signed_val.content.split(",");

        if &String::from(auth_comp.next()?) != username {
            return Err(InvalidUsername);
        }

        let auth_time = auth_comp.next()?.parse::<u128>()?;
        let range = auth_time..auth_time + Config::get_auth_delay().clone().parse::<u128>()?;

        if !range.contains(&Time::get_time()) {
            return Err(TimeStampOutOfBounds);
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
    use crate::auth_service::{AuthGenError, AuthService, AuthServiceImpl, VerificationError};
    use crate::clock::ClockImpl;
    use crate::crypto::{DecryptedValue, PubKeyCryptoService, RsaCryptoService, SignedValue};

    use rand::rngs::OsRng;
    use rsa::{RSAPrivateKey, RSAPublicKey};
    use std::mem::discriminant;

    #[test]
    fn test_auth_inverse_property() {
        let private_key = RSAPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key = RSAPublicKey::from(private_key.clone());

        let username = String::from("Smail");
        let auth =
            AuthServiceImpl::<ClockImpl, RsaCryptoService>::generate_auth(&private_key, &username)
                .unwrap();
        AuthServiceImpl::<ClockImpl, RsaCryptoService>::verify_auth(&auth, &public_key, &username)
            .unwrap()
    }

    #[test]
    fn test_auth_invalid_username() {
        let private_key = RSAPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key = RSAPublicKey::from(private_key.clone());

        let username = String::from(",");
        let auth =
            AuthServiceImpl::<ClockImpl, RsaCryptoService>::generate_auth(&private_key, &username)
                .unwrap();

        let result = discriminant(
            &AuthServiceImpl::<ClockImpl, RsaCryptoService>::verify_auth(
                &auth,
                &public_key,
                &String::from("Hamza"),
            )
            .unwrap_err(),
        );
        let error = discriminant(&VerificationError::InvalidUsername);

        assert_eq!(result, error);
    }
}
