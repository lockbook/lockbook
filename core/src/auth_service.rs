use crate::auth_service::VerificationError::{InvalidUsername, TimeStampOutOfBounds, TimeStampParseFailure, IncompleteAuth, InvalidTimeStamp, CryptoVerificationError, AuthDeserializationError};
use crate::clock::Clock;
use crate::crypto::{PubKeyCryptoService, SignedValue, SignatureVerificationFailed};
use crate::error_enum;

use rsa::{RSAPrivateKey, RSAPublicKey, PublicKey};

use serde::export::PhantomData;
use std::num::ParseIntError;
use std::option::NoneError;
use std::time::SystemTimeError;
use serde::de::IntoDeserializer;

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    CryptoVerificationError(SignatureVerificationFailed),
    IncompleteAuth(NoneError),
    InvalidTimeStamp(SystemTimeError),
    AuthDeserializationError(serde_json::error::Error),
    InvalidUsername
}

impl From<ParseIntError> for VerificationError {
    fn from(e: ParseIntError) -> Self { TimeStampParseFailure(e) }
}

impl From<SignatureVerificationFailed> for VerificationError {
    fn from(e: SignatureVerificationFailed) -> Self { CryptoVerificationError(e) }
}

impl From<NoneError> for VerificationError {
    fn from(e: NoneError) -> Self { IncompleteAuth(e) }
}

impl From<SystemTimeError> for VerificationError {
    fn from(e: SystemTimeError) -> Self { InvalidTimeStamp(e) }
}

impl From<serde_json::error::Error> for VerificationError {
    fn from(e: serde_json::error::Error) -> Self { AuthDeserializationError(e) }
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
        username: &String
    ) -> Result<(), VerificationError>;
    fn generate_auth(
        private_key: &RSAPrivateKey,
        username: &String
    ) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl<Time: Clock, Crypto: PubKeyCryptoService> { //better name
clock: PhantomData<Time>,
    crypto: PhantomData<Crypto>
}

impl<Time: Clock, Crypto: PubKeyCryptoService> AuthService for AuthServiceImpl<Time, Crypto>
{
    fn verify_auth(
        auth: &String,
        public_key: &RSAPublicKey,
        username: &String
    ) -> Result<(), VerificationError> {
        let signed_val = serde_json::from_str::<SignedValue>(&String::from(auth))?;

        Crypto::verify(&public_key, &signed_val)?;
        let mut auth_comp = signed_val.content.split(",");
        let real_time = Time::get_time();

        if &String::from(auth_comp.next()?) != username {
            return Err(InvalidUsername);
        }

        let auth_time = auth_comp.next()?.parse::<u128>()?;
        let range = auth_time..auth_time + 50;

        if !range.contains(&real_time) {
            return Err(TimeStampOutOfBounds);
        }
        Ok(())
    }

    fn generate_auth(
        private_key: &RSAPrivateKey,
        username: &String,
    ) -> Result<String, AuthGenError> {
        let to_sign = format!("{},{}",
                              username,
                              Time::get_time().to_string());

        Ok(serde_json::to_string(&Crypto::sign(&private_key, to_sign)?.signature)?)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::auth_service::{AuthServiceImpl, AuthService, VerificationError};
    use crate::crypto::{RsaCryptoService, DecryptedValue, PubKeyCryptoService, SignedValue};
    use std::mem::discriminant;
    use rsa::{RSAPublicKey, PublicKey, BigUint, RSAPrivateKey};
    use crate::clock::ClockImpl;
    use rand::rngs::OsRng;

    fn string_to_biguint(s: String) -> BigUint {
        BigUint::from_bytes_be(s.as_bytes())
    }

    #[test]
    fn test_auth_inverse_property() {
        let private_key = RSAPrivateKey::new( &mut OsRng, 2048).unwrap();
        let public_key = RSAPublicKey::from(private_key);

        let username = String::from("Smail");
        let auth = AuthServiceImpl::<ClockImpl, RsaCryptoService>::generate_auth(&private_key, &username).unwrap();

        AuthServiceImpl::<ClockImpl, RsaCryptoService>::verify_auth(&auth, &public_key, &username).unwrap()
    }

    #[test]
    fn test_auth_invalid_username() {
        let private_key = RSAPrivateKey::new( &mut OsRng, 2048).unwrap();
        let public_key = RSAPublicKey::from(private_key);

        let username = String::from("Smail");
        let auth = AuthServiceImpl::<ClockImpl, RsaCryptoService>::generate_auth(&private_key, &username).unwrap();


        let result = discriminant(&AuthServiceImpl::verify_auth(&keys.public_key, &public_key, &auth).unwrap_err());
        let error = discriminant(&VerificationError::InvalidUsername);

        assert_eq!(result, error);
    }
}