use core::num::ParseIntError;
use std::option::NoneError;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::SystemTimeError;

use crate::auth_service::VerificationError::{DecryptionFailure, IncompleteAuth, InvalidTimeStamp, InvalidUsername, TimeStampOutOfBounds, TimeStampParseFailure};
use crate::crypto::{CryptoService, DecryptedValue, EncryptedValue, KeyPair, PublicKey, RsaCryptoService};
use crate::crypto::DecryptionError;
use crate::crypto::EncryptionError;
use crate::error_enum;

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    DecryptionFailure(DecryptionError),
    IncompleteAuth(NoneError),
    InvalidTimeStamp(SystemTimeError),
    InvalidUsername,
    TimeStampOutOfBounds,
}

impl From<ParseIntError> for VerificationError {
    fn from(e: ParseIntError) -> Self { TimeStampParseFailure(e) }
}

impl From<DecryptionError> for VerificationError {
    fn from(e: DecryptionError) -> Self { DecryptionFailure(e) }
}

impl From<NoneError> for VerificationError {
    fn from(e: NoneError) -> Self { IncompleteAuth(e) }
}

impl From<SystemTimeError> for VerificationError {
    fn from(e: SystemTimeError) -> Self { InvalidTimeStamp(e) }
}

error_enum! {
    enum AuthGenError {
        AuthEncryptionFailure(EncryptionError),
        InvalidTimeStamp(SystemTimeError)
    }
}

pub trait AuthService {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), VerificationError>;
    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl;

impl AuthService for AuthServiceImpl {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), VerificationError> {
        let decrypt_val = RsaCryptoService::decrypt_public(
            &PublicKey {
                n: pub_key.n.clone(),
                e: pub_key.e.clone(),
            },
            &EncryptedValue {
                garbage: auth.clone(),
            },
        )?;

        let mut auth_comp = decrypt_val.secret.split(",");
        let real_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let auth_time = auth_comp.next()?.parse::<u128>()?;

        if &String::from(auth_comp.next()?) != username {
            return Err(InvalidUsername);
        }

        let range = auth_time..auth_time + 50;

        if !range.contains(&real_time) {
            return Err(TimeStampOutOfBounds);
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthGenError> {
        let decrypted = format!("{},{}",
                                username,
                                SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().to_string());

        Ok(RsaCryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted })?.garbage)
    }
}